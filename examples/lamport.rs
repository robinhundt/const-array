//! Lamport one-time signatures (Lamport 1979), generic over any `Digest`.
//!
//! Shows:
//! - sizes that are *products* of a digest's output size: a key holds two
//!   digest outputs per bit of a digest output, i.e. `2 * 8 * n` outputs of `n`
//!   bytes. `Prod` does this type-level multiplication without any bounds such
//!   as typenum's `Mul`.
//! - viewing a flat key as nested chunks with `as_chunks`: first as a list of
//!   digest outputs, then as a list of pairs of them,
//! - flattening arrays of arrays again with `from_chunks`, e.g. to turn the
//!   bytes of a digest output into its bits.
//!
//! Run with `cargo run --example lamport`.

#[allow(dead_code)]
mod crypto;

use const_array::{Array, ArrayLen, Len, Prod, same_len};
use crypto::{
    fake::{FakeSha256, FakeSha512},
    traits::{Digest, Error, Output, ct_eq},
};

/// One entry per bit of a digest output of `D`.
type Bits<D> = Prod<<D as Digest>::OutputSize, Len<8>>;

/// Keys are two digest outputs of `D` per bit of the message hash. The first
/// is revealed if the bit is 0, the second if it is 1.
type KeySize<D> = Prod<Prod<Bits<D>, Len<2>>, <D as Digest>::OutputSize>;

/// A signature reveals one digest output of `D` per bit of the message hash.
type SignatureSize<D> = Prod<Bits<D>, <D as Digest>::OutputSize>;

pub type SigningKey<D> = Array<u8, KeySize<D>>;
pub type VerifyingKey<D> = Array<u8, KeySize<D>>;
pub type Signature<D> = Array<u8, SignatureSize<D>>;

/// The bits of the hash of `msg`, least significant bit of each byte first.
fn message_bits<D: Digest>(msg: &[u8]) -> Array<bool, Bits<D>> {
    let bytes = D::digest(msg).map_ref(|byte| Array::from_fn(|j| byte >> j & 1 == 1));
    Array::from_chunks(bytes)
}

/// Hash each digest-sized chunk of `xs`.
fn hash_chunks<D: Digest, N: ArrayLen>(
    xs: &Array<u8, Prod<N, D::OutputSize>>,
) -> Array<Output<D>, N> {
    xs.as_chunks().map_ref(|x| D::digest(x))
}

/// Pick the digest output of each pair that the corresponding bit selects.
fn select<D: Digest>(
    key: &SigningKey<D>,
    bits: &Array<bool, Bits<D>>,
) -> Array<Output<D>, Bits<D>> {
    // A flat key, viewed as `Bits<D>` pairs of digest outputs. Both views are
    // free, and the pairs can't be indexed out of bounds.
    let pairs = key.as_chunks().as_chunks();
    pairs.zip_with(bits, |pair, &bit| pair[usize::from(bit)].clone())
}

/// Derive a signing key from a seed.
pub fn signing_key<D: Digest>(seed: &Output<D>) -> SigningKey<D> {
    let outputs = Array::from_fn(|i| {
        D::default()
            .chain(seed)
            .chain(&(i as u64).to_be_bytes())
            .finalize()
    });
    Array::from_chunks(outputs)
}

pub fn verifying_key<D: Digest>(sk: &SigningKey<D>) -> VerifyingKey<D> {
    Array::from_chunks(hash_chunks::<D, _>(sk))
}

/// Sign `msg`. A signing key must only ever sign a single message.
pub fn sign<D: Digest>(sk: &SigningKey<D>, msg: &[u8]) -> Signature<D> {
    Array::from_chunks(select::<D>(sk, &message_bits::<D>(msg)))
}

pub fn verify<D: Digest>(
    vk: &VerifyingKey<D>,
    msg: &[u8],
    sig: &Signature<D>,
) -> Result<(), Error> {
    let revealed = Array::from_chunks(hash_chunks::<D, _>(sig));
    let expected = Array::from_chunks(select::<D>(vk, &message_bits::<D>(msg)));
    ct_eq(&revealed, &expected).then_some(()).ok_or(Error)
}

fn main() {
    let seed = Array::from([3; 32]);
    let sk = signing_key::<FakeSha256>(&seed);
    let vk = verifying_key::<FakeSha256>(&sk);
    let sig = sign::<FakeSha256>(&sk, b"message");
    assert_eq!(verify::<FakeSha256>(&vk, b"message", &sig), Ok(()));
    assert_eq!(verify::<FakeSha256>(&vk, b"other", &sig), Err(Error));

    println!("first revealed output: {:02x?}", sig.as_chunks()[0]);

    // With concrete types, the sizes can be flattened with a compile-time
    // checked cast: 256 bits, each revealing 32 bytes.
    let wire: Array<u8, Len<8192>> = sig.cast(same_len!(SignatureSize<FakeSha256>, Len<8192>));
    println!("key: {} bytes, signature: {} bytes", vk.len(), wire.len());

    // The same code works with a 64 byte digest, with keys of 64 KiB.
    let seed = Array::from([5; 64]);
    let sk = signing_key::<FakeSha512>(&seed);
    let vk = verifying_key::<FakeSha512>(&sk);
    let sig = sign::<FakeSha512>(&sk, b"message");
    assert_eq!(verify::<FakeSha512>(&vk, b"message", &sig), Ok(()));
    println!("key: {} bytes, signature: {} bytes", vk.len(), sig.len());
}
