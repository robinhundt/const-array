//! Cryptographic traits built on [`Array`].
//!
//! Every fixed size is an associated [`ArrayLen`] type. Generic code can
//! combine these sizes with [`Sum`] without any `where` clauses on type-level
//! arithmetic, and split the combined arrays again for free.
//!
//! Following the crate's API convention, concrete primitives (see
//! [`fake`](super::fake)) declare flat sizes such as `Len<32>`, while generic
//! constructions such as `EncryptThenMac<C, M>` declare the `Sum` of their
//! parts' sizes.

use const_array::{Array, ArrayLen, AtMost, Sum};

/// A hash function.
pub trait Digest: Default + Clone {
    /// Size of the hash output.
    type OutputSize: ArrayLen;
    /// Size of the internal block (needed e.g. by HMAC).
    type BlockSize: ArrayLen;
    /// Proof that the output fits into one block, which HMAC relies on.
    ///
    /// A requirement of the trait, stated as a proof instead of a bound.
    /// Implementations with concrete sizes create it with `at_most!`, which
    /// fails during `cargo check` if the output is too long. Generic code can
    /// rely on it without repeating a bound like typenum's
    /// `OutputSize: IsLessOrEqual<BlockSize>` in every signature.
    const OUTPUT_FITS_BLOCK: AtMost<Self::OutputSize, Self::BlockSize>;

    fn update(&mut self, data: &[u8]);

    fn finalize(self) -> Output<Self>;

    /// Builder-style [`Digest::update`].
    fn chain(mut self, data: &[u8]) -> Self {
        self.update(data);
        self
    }

    /// Hash `data` in one go.
    fn digest(data: &[u8]) -> Output<Self> {
        Self::default().chain(data).finalize()
    }
}

/// The output of the digest `D`.
pub type Output<D> = Array<u8, <D as Digest>::OutputSize>;

/// A message authentication code with a fixed-size key.
pub trait Mac: Sized {
    type KeySize: ArrayLen;
    type TagSize: ArrayLen;

    fn new(key: &Array<u8, Self::KeySize>) -> Self;

    fn update(&mut self, data: &[u8]);

    fn finalize(self) -> Array<u8, Self::TagSize>;

    /// Check `tag` against the computed tag.
    fn verify(self, tag: &Array<u8, Self::TagSize>) -> Result<(), Error> {
        ct_eq(&self.finalize(), tag).then_some(()).ok_or(Error)
    }
}

/// A stream cipher: XORs a keystream derived from key and nonce into data.
pub trait StreamCipher {
    type KeySize: ArrayLen;
    type NonceSize: ArrayLen;

    fn new(key: &Array<u8, Self::KeySize>, nonce: &Array<u8, Self::NonceSize>) -> Self;

    fn apply_keystream(&mut self, buf: &mut [u8]);
}

/// Authenticated encryption with associated data.
pub trait Aead: Sized {
    type KeySize: ArrayLen;
    type NonceSize: ArrayLen;
    type TagSize: ArrayLen;

    fn new(key: &Key<Self>) -> Self;

    /// Encrypt `buf` in place and return the authentication tag.
    fn encrypt_in_place_detached(
        &self,
        nonce: &Nonce<Self>,
        aad: &[u8],
        buf: &mut [u8],
    ) -> Tag<Self>;

    /// Verify `tag` and decrypt `buf` in place.
    fn decrypt_in_place_detached(
        &self,
        nonce: &Nonce<Self>,
        aad: &[u8],
        buf: &mut [u8],
        tag: &Tag<Self>,
    ) -> Result<(), Error>;

    /// Encrypt a fixed-size message. The size of the result, message plus
    /// tag, is part of the type.
    fn seal<P: ArrayLen>(
        &self,
        nonce: &Nonce<Self>,
        aad: &[u8],
        mut msg: Array<u8, P>,
    ) -> Array<u8, Sum<P, Self::TagSize>> {
        let tag = self.encrypt_in_place_detached(nonce, aad, &mut msg);
        Array::concat(msg, tag)
    }

    /// Decrypt a message sealed with [`Aead::seal`]. The plaintext size `P`
    /// is usually inferred from the ciphertext type.
    fn open<P: ArrayLen>(
        &self,
        nonce: &Nonce<Self>,
        aad: &[u8],
        sealed: Array<u8, Sum<P, Self::TagSize>>,
    ) -> Result<Array<u8, P>, Error> {
        let (mut msg, tag) = sealed.parts();
        self.decrypt_in_place_detached(nonce, aad, &mut msg, &tag)?;
        Ok(msg)
    }
}

pub type Key<A> = Array<u8, <A as Aead>::KeySize>;
pub type Nonce<A> = Array<u8, <A as Aead>::NonceSize>;
pub type Tag<A> = Array<u8, <A as Aead>::TagSize>;

/// A key encapsulation mechanism.
pub trait Kem {
    type EncapsulationKeySize: ArrayLen;
    type DecapsulationKeySize: ArrayLen;
    type CiphertextSize: ArrayLen;
    type SharedSecretSize: ArrayLen;

    fn generate(rng: &mut impl Rng) -> (DecapsulationKey<Self>, EncapsulationKey<Self>);

    fn encapsulate(
        ek: &EncapsulationKey<Self>,
        rng: &mut impl Rng,
    ) -> (Ciphertext<Self>, SharedSecret<Self>);

    fn decapsulate(dk: &DecapsulationKey<Self>, ct: &Ciphertext<Self>) -> SharedSecret<Self>;
}

pub type EncapsulationKey<K> = Array<u8, <K as Kem>::EncapsulationKeySize>;
pub type DecapsulationKey<K> = Array<u8, <K as Kem>::DecapsulationKeySize>;
pub type Ciphertext<K> = Array<u8, <K as Kem>::CiphertextSize>;
pub type SharedSecret<K> = Array<u8, <K as Kem>::SharedSecretSize>;

/// A source of randomness.
pub trait Rng {
    fn fill_bytes(&mut self, buf: &mut [u8]);

    /// A random array of any size.
    fn random<S: ArrayLen>(&mut self) -> Array<u8, S> {
        let mut out = Array::default();
        self.fill_bytes(&mut out);
        out
    }
}

/// Opaque error for failed verification or decryption.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Error;

/// Compare two arrays without branching on their contents.
///
/// Both arrays must have the same size, so there is no length check that
/// could be forgotten, and no `zip` that could silently truncate. Real code
/// should use a crate like `subtle` instead of relying on the optimizer.
pub fn ct_eq<S: ArrayLen>(a: &Array<u8, S>, b: &Array<u8, S>) -> bool {
    a.zip_with(b, |x, y| x ^ y).iter().fold(0, |acc, d| acc | d) == 0
}
