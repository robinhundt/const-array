//! **Insecure** stand-ins for cryptographic primitives.
//!
//! None of these provide any security. They only have the sizes of the real
//! primitives they are named after and enough (trivial) behavior that the
//! examples round-trip: decryption undoes encryption, both sides of a KEM
//! agree on the secret, and so on.

use const_array::{Array, AtMost, Len};

use super::traits::{
    Ciphertext, DecapsulationKey, Digest, EncapsulationKey, Kem, Output, Rng, SharedSecret,
    StreamCipher,
};

/// Not a hash function: an FNV-1a accumulator stretched to `OUT` bytes.
#[derive(Clone)]
pub struct FakeDigest<const OUT: usize, const BLOCK: usize> {
    state: u64,
}

impl<const OUT: usize, const BLOCK: usize> Default for FakeDigest<OUT, BLOCK> {
    fn default() -> Self {
        Self {
            state: 0xcbf2_9ce4_8422_2325,
        }
    }
}

impl<const OUT: usize, const BLOCK: usize> Digest for FakeDigest<OUT, BLOCK> {
    type OutputSize = Len<OUT>;
    type BlockSize = Len<BLOCK>;
    // The sizes are generic, so `at_most!` can't be used. A digest with a
    // concrete size would use `at_most!(Len<32>, Len<64>)` instead.
    const OUTPUT_FITS_BLOCK: AtMost<Len<OUT>, Len<BLOCK>> = AtMost::checked();

    fn update(&mut self, data: &[u8]) {
        for &b in data {
            self.state = (self.state ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
        }
    }

    fn finalize(self) -> Output<Self> {
        Array::from_fn(|i| (self.state.rotate_left(8 * i as u32) as u8) ^ i as u8)
    }
}

/// Has the sizes of SHA-256.
pub type FakeSha256 = FakeDigest<32, 64>;
/// Has the sizes of SHA-512.
pub type FakeSha512 = FakeDigest<64, 128>;

/// Not a cipher: XORs `key[i] ^ nonce[i] ^ i` into the data. Has the key and
/// nonce sizes `KEY` and `NONCE`.
pub struct FakeCipher<const KEY: usize, const NONCE: usize> {
    key: Array<u8, Len<KEY>>,
    nonce: Array<u8, Len<NONCE>>,
    pos: usize,
}

impl<const KEY: usize, const NONCE: usize> StreamCipher for FakeCipher<KEY, NONCE> {
    type KeySize = Len<KEY>;
    type NonceSize = Len<NONCE>;

    fn new(key: &Array<u8, Len<KEY>>, nonce: &Array<u8, Len<NONCE>>) -> Self {
        Self {
            key: *key,
            nonce: *nonce,
            pos: 0,
        }
    }

    fn apply_keystream(&mut self, buf: &mut [u8]) {
        for b in buf {
            let k = self.key.get(self.pos % KEY.max(1)).copied().unwrap_or(0);
            let n = self
                .nonce
                .get(self.pos % NONCE.max(1))
                .copied()
                .unwrap_or(0);
            *b ^= k ^ n ^ self.pos as u8;
            self.pos += 1;
        }
    }
}

/// Has the sizes of ChaCha20.
pub type FakeChaCha20 = FakeCipher<32, 12>;

/// Not a KEM: the encapsulation key *is* the decapsulation key, and the
/// ciphertext is the shared secret XORed with it. Has the key, ciphertext
/// and secret sizes `EK`, `CT` and `SS`.
pub struct FakeKem<const EK: usize, const CT: usize, const SS: usize>;

impl<const EK: usize, const CT: usize, const SS: usize> Kem for FakeKem<EK, CT, SS> {
    type EncapsulationKeySize = Len<EK>;
    type DecapsulationKeySize = Len<EK>;
    type CiphertextSize = Len<CT>;
    type SharedSecretSize = Len<SS>;

    fn generate(rng: &mut impl Rng) -> (DecapsulationKey<Self>, EncapsulationKey<Self>) {
        let dk: Array<u8, Len<EK>> = rng.random();
        (dk, dk)
    }

    fn encapsulate(
        ek: &EncapsulationKey<Self>,
        rng: &mut impl Rng,
    ) -> (Ciphertext<Self>, SharedSecret<Self>) {
        let ss: Array<u8, Len<SS>> = rng.random();
        let ct = Array::from_fn(|i| if i < SS { ss[i] ^ ek[i] } else { 0 });
        (ct, ss)
    }

    fn decapsulate(dk: &DecapsulationKey<Self>, ct: &Ciphertext<Self>) -> SharedSecret<Self> {
        Array::from_fn(|i| ct[i] ^ dk[i])
    }
}

/// Has the sizes of X25519 used as a KEM.
pub type FakeX25519 = FakeKem<32, 32, 32>;
/// Has the sizes of ML-KEM-768.
pub type FakeMlKem768 = FakeKem<1184, 1088, 32>;

/// Not random: a counter.
#[derive(Default)]
pub struct FakeRng(u8);

impl Rng for FakeRng {
    fn fill_bytes(&mut self, buf: &mut [u8]) {
        for b in buf {
            self.0 = self.0.wrapping_add(1);
            *b = self.0;
        }
    }
}
