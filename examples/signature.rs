//! Signatures, and connecting digests to signature schemes generically.
//!
//! Shows:
//! - a signature trait whose key, prehash and signature sizes are `ArrayLen`s.
//!   Following the crate's API convention, the ECDSA-like scheme declares its
//!   signature `r || s` as a flat `Len<64>` and splits it internally with a
//!   `same_len!` checked cast,
//! - requiring matching sizes with an associated type bound (`PrehashSize =
//!   D::OutputSize`): a mismatch is a type error,
//! - requiring matching *lengths* of differently structured sizes in generic
//!   code with a `SameLen` proof parameter, e.g. for Ed25519-style seed
//!   expansion, which splits a 64 byte digest into two 32 byte halves,
//! - the runtime alternative, `SameLen::try_new`.
//!
//! Run with `cargo run --example signature`.

#[allow(dead_code)]
mod crypto;

use const_array::{Array, ArrayLen, Len, SameLen, Sum, same_len};
use crypto::{
    fake::{FakeSha256, FakeSha512},
    traits::{Digest, Error, ct_eq},
};

/// A signature scheme that signs a fixed-size message hash.
pub trait PrehashSignature {
    type SigningKeySize: ArrayLen;
    type VerifyingKeySize: ArrayLen;
    type PrehashSize: ArrayLen;
    type SignatureSize: ArrayLen;

    fn verifying_key(sk: &SigningKey<Self>) -> VerifyingKey<Self>;

    fn sign_prehash(sk: &SigningKey<Self>, prehash: &Prehash<Self>) -> Signature<Self>;

    fn verify_prehash(
        vk: &VerifyingKey<Self>,
        prehash: &Prehash<Self>,
        sig: &Signature<Self>,
    ) -> Result<(), Error>;
}

pub type SigningKey<S> = Array<u8, <S as PrehashSignature>::SigningKeySize>;
pub type VerifyingKey<S> = Array<u8, <S as PrehashSignature>::VerifyingKeySize>;
pub type Prehash<S> = Array<u8, <S as PrehashSignature>::PrehashSize>;
pub type Signature<S> = Array<u8, <S as PrehashSignature>::SignatureSize>;

/// **Insecure** stand-in with the sizes of ECDSA over P-256. The verifying
/// key is the signing key, `r = sk ^ prehash` and `s = prehash`.
pub struct FakeEcdsaP256;

type Scalar = Len<32>;
/// The internal structure of a signature, `r || s`. It is not part of the
/// public API, see `SignatureSize`.
type RS = Sum<Scalar, Scalar>;

impl PrehashSignature for FakeEcdsaP256 {
    type SigningKeySize = Scalar;
    type VerifyingKeySize = Scalar;
    type PrehashSize = Len<32>;
    /// `r || s`. The public size is flat, so callers and trait bounds can
    /// use `Len<64>`, and the implementation can change its internal
    /// structure without breaking them.
    type SignatureSize = Len<64>;

    fn verifying_key(sk: &SigningKey<Self>) -> VerifyingKey<Self> {
        *sk
    }

    fn sign_prehash(sk: &SigningKey<Self>, prehash: &Prehash<Self>) -> Signature<Self> {
        let r = sk.zip_with(prehash, |a, b| a ^ b);
        r.concat_with(*prehash, same_len!(RS, Len<64>))
    }

    fn verify_prehash(
        vk: &VerifyingKey<Self>,
        prehash: &Prehash<Self>,
        sig: &Signature<Self>,
    ) -> Result<(), Error> {
        // No offsets, no slicing, no runtime length checks.
        let (r, s) = sig.split_ref_with(same_len!(Len<64>, RS));
        let expected_r = vk.zip_with(prehash, |a, b| a ^ b);
        (ct_eq(r, &expected_r) & ct_eq(s, prehash))
            .then_some(())
            .ok_or(Error)
    }
}

/// Hash-then-sign with any digest whose output size *is* the prehash size of
/// the signature scheme. `sign::<FakeSha512, FakeEcdsaP256>` is a type error.
fn sign<D, S>(sk: &SigningKey<S>, msg: &[u8]) -> Signature<S>
where
    D: Digest,
    S: PrehashSignature<PrehashSize = D::OutputSize>,
{
    S::sign_prehash(sk, &D::digest(msg))
}

fn verify<D, S>(vk: &VerifyingKey<S>, msg: &[u8], sig: &Signature<S>) -> Result<(), Error>
where
    D: Digest,
    S: PrehashSignature<PrehashSize = D::OutputSize>,
{
    S::verify_prehash(vk, &D::digest(msg), sig)
}

/// The two halves of an expanded Ed25519-style seed.
struct ExpandedSeed {
    scalar: Array<u8, Len<32>>,
    prefix: Array<u8, Len<32>>,
}

/// Ed25519-style seed expansion with any 64 byte digest.
///
/// Digests declare their output as a plain size such as `Len<64>`, which a
/// type equality bound could not match against `Sum<Len<32>, Len<32>>`. The
/// proof parameter requires the same *length* instead, and callers with
/// concrete types create it with `same_len!`.
fn expand_seed<D: Digest>(
    seed: &Array<u8, Len<32>>,
    proof: SameLen<D::OutputSize, Sum<Len<32>, Len<32>>>,
) -> ExpandedSeed {
    let (mut scalar, prefix) = D::digest(seed).parts_with(proof);
    // Clamping, as in Ed25519.
    scalar[0] &= 248;
    scalar[31] &= 127;
    scalar[31] |= 64;
    ExpandedSeed { scalar, prefix }
}

/// The same with a runtime check, for when a proof can't be threaded through.
/// The check is on constants, so it is optimized away.
fn try_expand_seed<D: Digest>(seed: &Array<u8, Len<32>>) -> Option<ExpandedSeed> {
    let proof = SameLen::try_new()?;
    Some(expand_seed::<D>(seed, proof))
}

fn main() {
    let sk: SigningKey<FakeEcdsaP256> = Array::from([7; 32]);
    let vk = FakeEcdsaP256::verifying_key(&sk);

    let sig = sign::<FakeSha256, FakeEcdsaP256>(&sk, b"message");
    // Callers see a plain 64 byte signature, which a bound like
    // `SignatureSize = Len<64>` or a `[u8; 64]` wire format can match.
    let wire: [u8; 64] = sig.into_array();
    println!("sig: {:02x?}", wire);
    let sig = Array::new(wire);
    assert_eq!(
        verify::<FakeSha256, FakeEcdsaP256>(&vk, b"message", &sig),
        Ok(())
    );
    assert_eq!(
        verify::<FakeSha256, FakeEcdsaP256>(&vk, b"other", &sig),
        Err(Error)
    );

    // Seed expansion with a 64 byte digest. `same_len!` checks at compile
    // time that the digest output has 64 bytes.
    let seed = Array::from([1; 32]);
    let expanded = expand_seed::<FakeSha512>(
        &seed,
        same_len!(<FakeSha512 as Digest>::OutputSize, Sum<Len<32>, Len<32>>),
    );
    println!("scalar: {:02x?}", expanded.scalar);
    println!("prefix: {:02x?}", expanded.prefix);

    // With a 32 byte digest, `same_len!` would fail to compile. The runtime
    // check reports the mismatch instead.
    assert!(try_expand_seed::<FakeSha512>(&seed).is_some());
    assert!(try_expand_seed::<FakeSha256>(&seed).is_none());
}
