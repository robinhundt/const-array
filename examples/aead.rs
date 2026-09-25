//! AEAD, generic compositions over it, and a fixed-size message API.
//!
//! Shows:
//! - `EncryptThenMac<C, M>`: an AEAD composed of any stream cipher and any MAC.
//!   Its key size is `Sum<C::KeySize, M::KeySize>`, and splitting the key into
//!   its parts needs no length check.
//! - `Channel<A>`: TLS 1.3 style per-record nonces for any AEAD, computed as
//!   `iv ^ counter` over arrays of the same (generic) size.
//! - `seal`/`open` on fixed-size messages, where the ciphertext type records
//!   the plaintext and tag sizes.
//! - sending a fixed number of fixed-size records at once: `Prod<N, R>`
//!   plaintext bytes become `Prod<N, Sum<R, A::TagSize>>` ciphertext bytes.
//! - building a nonce from structured parts with `concat` and `same_len!`.
//!
//! Run with `cargo run --example aead`.

#[allow(dead_code)]
mod crypto;

use const_array::{Array, ArrayLen, Len, Prod, Sum, same_len};
use crypto::{
    fake::{FakeChaCha20, FakeCipher, FakeSha256},
    hmac::Hmac,
    traits::{Aead, Error, Key, Mac, Nonce, StreamCipher, Tag, ct_eq},
};

/// Encrypt-then-MAC composition of a stream cipher `C` and a MAC `M`.
pub struct EncryptThenMac<C: StreamCipher, M: Mac> {
    cipher_key: Array<u8, C::KeySize>,
    mac_key: Array<u8, M::KeySize>,
}

impl<C: StreamCipher, M: Mac> EncryptThenMac<C, M> {
    fn tag(
        &self,
        nonce: &Array<u8, C::NonceSize>,
        aad: &[u8],
        ciphertext: &[u8],
    ) -> Array<u8, M::TagSize> {
        let mut mac = M::new(&self.mac_key);
        mac.update(nonce);
        mac.update(aad);
        mac.update(ciphertext);
        mac.update(&(aad.len() as u64).to_be_bytes());
        mac.update(&(ciphertext.len() as u64).to_be_bytes());
        mac.finalize()
    }
}

impl<C: StreamCipher, M: Mac> Aead for EncryptThenMac<C, M> {
    // The key is the cipher key followed by the MAC key. `Sum` does the
    // type-level arithmetic, without any bounds on `C` or `M`. As a generic
    // construction, its size can't be flattened to a `Len<N>`, so it
    // exposes the `Sum`.
    type KeySize = Sum<C::KeySize, M::KeySize>;
    type NonceSize = C::NonceSize;
    type TagSize = M::TagSize;

    fn new(key: &Key<Self>) -> Self {
        let (cipher_key, mac_key) = key.split_ref();
        Self {
            cipher_key: cipher_key.clone(),
            mac_key: mac_key.clone(),
        }
    }

    fn encrypt_in_place_detached(
        &self,
        nonce: &Nonce<Self>,
        aad: &[u8],
        buf: &mut [u8],
    ) -> Tag<Self> {
        C::new(&self.cipher_key, nonce).apply_keystream(buf);
        self.tag(nonce, aad, buf)
    }

    fn decrypt_in_place_detached(
        &self,
        nonce: &Nonce<Self>,
        aad: &[u8],
        buf: &mut [u8],
        tag: &Tag<Self>,
    ) -> Result<(), Error> {
        if !ct_eq(&self.tag(nonce, aad, buf), tag) {
            return Err(Error);
        }
        C::new(&self.cipher_key, nonce).apply_keystream(buf);
        Ok(())
    }
}

/// `N` records of `R` bytes, each sealed with the AEAD `A` and followed by its
/// tag.
pub type SealedRecords<A, N, R> = Array<u8, Prod<N, Sum<R, <A as Aead>::TagSize>>>;

/// A record layer for any AEAD `A`. As in TLS 1.3, the nonce of each record
/// is the static IV XORed with the record counter.
pub struct Channel<A: Aead> {
    aead: A,
    iv: Nonce<A>,
    counter: u64,
}

impl<A: Aead> Channel<A> {
    pub fn new(key: &Key<A>, iv: Nonce<A>) -> Self {
        Self {
            aead: A::new(key),
            iv,
            counter: 0,
        }
    }

    /// The nonce of the next record.
    ///
    /// Fails once the counter no longer fits into the nonce, which happens
    /// after `2^(8 * n)` records for an `n < 8` byte nonce, and after `2^64 -
    /// 1` records otherwise. Truncating the counter instead would reuse
    /// nonces.
    fn next_nonce(&mut self) -> Result<Nonce<A>, Error> {
        let len = A::NonceSize::USIZE;
        if len < 8 && self.counter >> (8 * len) != 0 {
            return Err(Error);
        }
        // The counter, big-endian and aligned to the end of the nonce. Bytes
        // before it are zero, and its leading zero bytes may be cut off if the
        // nonce is shorter than 8 bytes, as checked above.
        let counter = self.counter.to_be_bytes();
        let padded: Nonce<A> = Array::from_fn(|i| {
            let from_end = len - 1 - i;
            if from_end < 8 {
                counter[7 - from_end]
            } else {
                0
            }
        });
        self.counter = self.counter.checked_add(1).ok_or(Error)?;
        // Both operands have the size `A::NonceSize`, whatever it is.
        Ok(self.iv.zip_with(&padded, |a, b| a ^ b))
    }

    /// Encrypt a record in place and return its tag.
    /// Fails if the channel has run out of nonces.
    pub fn send(&mut self, record: &mut [u8]) -> Result<Tag<A>, Error> {
        let nonce = self.next_nonce()?;
        Ok(self
            .aead
            .encrypt_in_place_detached(&nonce, b"record", record))
    }

    /// Verify and decrypt a record in place.
    pub fn receive(&mut self, record: &mut [u8], tag: &Tag<A>) -> Result<(), Error> {
        let nonce = self.next_nonce()?;
        self.aead
            .decrypt_in_place_detached(&nonce, b"record", record, tag)
    }

    /// Seal `N` records of `R` bytes each. Each sealed record is followed by
    /// its tag.
    pub fn send_records<N: ArrayLen, R: ArrayLen>(
        &mut self,
        records: Array<u8, Prod<N, R>>,
    ) -> Result<SealedRecords<A, N, R>, Error> {
        let mut sealed = Array::<Array<u8, Sum<R, A::TagSize>>, N>::default();
        // Both have `N` elements, so `zip` can't skip a record.
        for (out, record) in sealed.iter_mut().zip(records.into_chunks()) {
            let nonce = self.next_nonce()?;
            *out = self.aead.seal(&nonce, b"record", record);
        }
        Ok(Array::from_chunks(sealed))
    }

    /// Verify and open `N` records sealed with [`Channel::send_records`].
    /// The record size `R` is usually inferred from the sealed type.
    pub fn receive_records<N: ArrayLen, R: ArrayLen>(
        &mut self,
        sealed: SealedRecords<A, N, R>,
    ) -> Result<Array<u8, Prod<N, R>>, Error> {
        let mut records = Array::<Array<u8, R>, N>::default();
        for (out, record) in records.iter_mut().zip(sealed.into_chunks()) {
            let nonce = self.next_nonce()?;
            *out = self.aead.open(&nonce, b"record", record)?;
        }
        Ok(Array::from_chunks(records))
    }
}

/// The concrete AEAD used below.
type MyAead = EncryptThenMac<FakeChaCha20, Hmac<FakeSha256>>;

/// A fixed-size message, e.g. a key to be wrapped.
type WrappedKey = Len<32>;

fn main() {
    // `MyAead` has a 64 byte key, made of two 32 byte parts. Its user knows
    // the concrete types, so a master key of plain `Len<64>` is converted with
    // a compile-time checked cast.
    let master_key: Array<u8, Len<64>> = Array::from_fn(|i| (i * 7 + 3) as u8);
    let key: Key<MyAead> = master_key.cast(same_len!(Len<64>, <MyAead as Aead>::KeySize));
    let aead = MyAead::new(&key);

    // The cipher wants a 12 byte nonce. Build it from a 4 byte connection id
    // and an 8 byte counter.
    let nonce: Array<u8, Sum<Len<4>, Len<8>>> =
        Array::concat(Array::from(*b"conn"), Array::from(7u64.to_be_bytes()));
    let nonce: Nonce<MyAead> = nonce.cast(same_len!(Sum<Len<4>, Len<8>>, Len<12>));

    // Seal a fixed-size message. The result is exactly 32 + 32 bytes, which
    // is part of its type.
    let secret: Array<u8, WrappedKey> = Array::from([0x42; 32]);
    let sealed: Array<u8, Sum<WrappedKey, Len<32>>> = aead.seal(&nonce, b"wrap", secret);
    println!("sealed ({} bytes): {:02x?}", sealed.len(), sealed);

    // `open` infers the plaintext size from the sealed type.
    let opened = aead.open(&nonce, b"wrap", sealed).expect("authentic");
    assert_eq!(opened, secret);

    // Tampering is detected.
    let mut tampered = sealed;
    tampered[0] ^= 1;
    assert_eq!(
        aead.open::<WrappedKey>(&nonce, b"wrap", tampered),
        Err(Error)
    );

    // A channel generic over the AEAD, instantiated with `MyAead`.
    let iv = Array::from([9; 12]);
    let mut alice = Channel::<MyAead>::new(&key, iv);
    let mut bob = Channel::<MyAead>::new(&key, iv);
    for msg in ["hello", "bob"] {
        let mut record = msg.as_bytes().to_vec();
        let tag = alice.send(&mut record).expect("nonces left");
        bob.receive(&mut record, &tag).expect("authentic");
        assert_eq!(record, msg.as_bytes());
    }
    println!("channel ok");

    // Three records of 16 bytes each, sealed in one go. Each sealed record
    // has room for its 32 byte tag, which is part of the type.
    let records: Array<u8, Prod<Len<3>, Len<16>>> = Array::from_fn(|i| i as u8);
    let sealed = alice.send_records(records).expect("nonces left");
    let wire: Array<u8, Len<144>> =
        sealed.cast(same_len!(Prod<Len<3>, Sum<Len<16>, Len<32>>>, Len<144>));
    println!("sealed records ({} bytes)", wire.len());
    let opened = bob.receive_records(sealed).expect("authentic");
    assert_eq!(opened, records);

    // With a 1 byte nonce, a channel can send 256 records. Afterwards it
    // refuses to send, instead of reusing a nonce.
    type ShortNonceAead = EncryptThenMac<FakeCipher<32, 1>, Hmac<FakeSha256>>;
    let key = master_key.cast(same_len!(Len<64>, <ShortNonceAead as Aead>::KeySize));
    let mut channel = Channel::<ShortNonceAead>::new(&key, Array::from([0]));
    for _ in 0..256 {
        channel.send(&mut []).expect("nonces left");
    }
    assert_eq!(channel.send(&mut []), Err(Error));
    println!("short nonce channel ok");
}
