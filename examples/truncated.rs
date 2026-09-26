//! Truncated digests (like SHA-512/256), usable wherever a `Digest` is.
//!
//! Shows:
//! - a trait requirement stated as an associated `const` proof instead of a
//!   bound: each `TruncateTo<N>` impl proves that `N` fits into the digest's
//!   output, with `at_most!` over `Self`'s sizes, checked by `cargo check`,
//! - a generic wrapper that satisfies `Digest`'s own proof,
//!   `OUTPUT_FITS_BLOCK`, by composing the proofs it has with `trans`. This
//!   can't fail, so it needs neither a check nor a bound,
//! - consuming the proofs: `truncate` in the wrapper and HMAC's key padding, so
//!   `Hmac<Truncated<..>>` just works.
//!
//! Run with `cargo run --example truncated`.

#[allow(dead_code)]
mod crypto;

use std::marker::PhantomData;

use const_array::{ArrayLen, AtMost, Len, at_most};
use crypto::{
    fake::{FakeSha256, FakeSha512},
    hmac::Hmac,
    traits::{Digest, Mac, Output},
};

/// A digest whose output can be truncated to `N` bytes.
///
/// The requirement `N <= OutputSize` is a proof, not a bound, so it doesn't
/// have to be repeated by generic code that uses `Truncated<D, N>`.
trait TruncateTo<N: ArrayLen>: Digest {
    const FITS_OUTPUT: AtMost<N, Self::OutputSize>;
}

// One line per supported truncation. `Self` is concrete, so `at_most!`
// checks it during `cargo check`: `impl TruncateTo<Len<80>> for FakeSha512`
// fails with "attempt to compute `64_usize - 80_usize`, which would overflow".
impl TruncateTo<Len<32>> for FakeSha512 {
    const FITS_OUTPUT: AtMost<Len<32>, Self::OutputSize> = at_most!(Len<32>, Self::OutputSize);
}

impl TruncateTo<Len<28>> for FakeSha256 {
    const FITS_OUTPUT: AtMost<Len<28>, Self::OutputSize> = at_most!(Len<28>, Self::OutputSize);
}

/// `D` with its output truncated to `N` bytes. (Real truncated SHA-2 variants
/// also use their own IV.)
#[derive(Clone)]
struct Truncated<D, N>(D, PhantomData<N>);

impl<D: Default, N> Default for Truncated<D, N> {
    fn default() -> Self {
        Self(D::default(), PhantomData)
    }
}

impl<D: TruncateTo<N>, N: ArrayLen> Digest for Truncated<D, N> {
    type OutputSize = N;
    type BlockSize = D::BlockSize;
    // N <= D::OutputSize <= D::BlockSize. Generic, but built only from proofs
    // that already exist, so it can't fail.
    const OUTPUT_FITS_BLOCK: AtMost<N, D::BlockSize> = D::FITS_OUTPUT.trans(D::OUTPUT_FITS_BLOCK);

    fn update(&mut self, data: &[u8]) {
        self.0.update(data);
    }

    fn finalize(self) -> Output<Self> {
        self.0.finalize().truncate(D::FITS_OUTPUT)
    }
}

type FakeSha512_256 = Truncated<FakeSha512, Len<32>>;
type FakeSha224 = Truncated<FakeSha256, Len<28>>;

fn main() {
    let full = FakeSha512::digest(b"abc");
    let short = FakeSha512_256::digest(b"abc");
    assert_eq!(short[..], full[..32]);
    println!("FakeSha512_256: {} bytes", short.len());

    // HMAC pads over-long keys to a block with `OUTPUT_FITS_BLOCK`, which the
    // wrapper derived from its parts.
    let key = [0x0b; 200];
    let mut mac = Hmac::<FakeSha224>::new_from_slice(&key);
    mac.update(b"message");
    let tag = mac.finalize();
    println!("HMAC-FakeSha224 tag: {} bytes", tag.len());
}
