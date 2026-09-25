//! HKDF (RFC 5869), generic over any [`Digest`].
//!
//! Shows:
//! - a construction generic over a digest, whose key sizes come from the
//!   digest's associated [`ArrayLen`]s,
//! - `expand` generic over the output size, so a key schedule derives all of
//!   its keys in one call and splits them with `parts`,
//! - deriving keys for several epochs at once as a `Prod` and iterating over
//!   them with `into_chunks`,
//! - deriving keys for a generic cipher directly in its key type,
//! - bridging differently structured sizes with `same_len!` and `cast`.
//!
//! Run with `cargo run --example hkdf`.

#[allow(dead_code)]
mod crypto;

use const_array::{Array, ArrayLen, Len, Prod, Sum, same_len};
use crypto::{
    fake::{FakeChaCha20, FakeSha256, FakeSha512},
    hmac::Hmac,
    traits::{Digest, Mac, Output, StreamCipher},
};

/// HKDF with the digest `D`. Holds the pseudorandom key.
pub struct Hkdf<D: Digest> {
    prk: Output<D>,
}

/// The requested output is longer than `255 * D::OutputSize::USIZE`.
#[derive(Debug)]
pub struct InvalidLength;

impl<D: Digest> Hkdf<D> {
    /// HKDF-Extract. The salt defaults to a zero-filled digest output.
    pub fn extract(salt: Option<&[u8]>, ikm: &[u8]) -> Self {
        let zeros = Output::<D>::default();
        let mut mac = Hmac::<D>::new_from_slice(salt.unwrap_or(&zeros));
        mac.update(ikm);
        Self {
            prk: mac.finalize(),
        }
    }

    /// Use an existing pseudorandom key. Its size is fixed by the digest.
    pub fn from_prk(prk: Output<D>) -> Self {
        Self { prk }
    }

    /// HKDF-Expand into an array of any size `L`.
    ///
    /// Whether `L` is too long only depends on constants, so the check is
    /// optimized away. It can't be a compile error without introducing a
    /// post-monomorphization error.
    pub fn expand<L: ArrayLen>(&self, info: &[u8]) -> Result<Array<u8, L>, InvalidLength> {
        let mut okm = Array::<u8, L>::default();
        self.expand_into(info, &mut okm)?;
        Ok(okm)
    }

    /// HKDF-Expand into a slice of any length.
    pub fn expand_into(&self, info: &[u8], okm: &mut [u8]) -> Result<(), InvalidLength> {
        if okm.len() > 255 * D::OutputSize::USIZE {
            return Err(InvalidLength);
        }
        let mut prev: Option<Output<D>> = None;
        for (i, chunk) in okm.chunks_mut(D::OutputSize::USIZE).enumerate() {
            let mut mac = Hmac::<D>::new(&self.prk);
            if let Some(prev) = &prev {
                mac.update(prev);
            }
            mac.update(info);
            mac.update(&[i as u8 + 1]);
            let block = mac.finalize();
            chunk.copy_from_slice(&block[..chunk.len()]);
            prev = Some(block);
        }
        Ok(())
    }
}

// A TLS-like key schedule. The sizes are declared with their structure, so
// all keys are derived in one `expand` and splitting them needs no index
// arithmetic, no length checks and no `unwrap`.
type AesKey = Len<16>;
type Iv = Len<12>;
type Direction = Sum<AesKey, Iv>;
type SessionKeys = Sum<Direction, Direction>;

struct DirectionKeys {
    key: Array<u8, AesKey>,
    iv: Array<u8, Iv>,
}

fn session_keys<D: Digest>(shared_secret: &[u8]) -> (DirectionKeys, DirectionKeys) {
    let hkdf = Hkdf::<D>::extract(Some(b"example salt"), shared_secret);
    let keys: Array<u8, SessionKeys> = hkdf
        .expand(b"session keys")
        .expect("44 bytes is a valid length");
    let (client, server) = keys.parts();
    let split = |dir: Array<u8, Direction>| {
        let (key, iv) = dir.parts();
        DirectionKeys { key, iv }
    };
    (split(client), split(server))
}

// Keys for the next 4 epochs of a connection, derived up front. `Prod` is
// the size of `Epochs` chunks of `SessionKeys` each.
type Epochs = Len<4>;
type EpochKeys = Prod<Epochs, SessionKeys>;

fn epoch_keys<D: Digest>(shared_secret: &[u8]) -> Array<Array<u8, SessionKeys>, Epochs> {
    let hkdf = Hkdf::<D>::extract(Some(b"example salt"), shared_secret);
    let keys: Array<u8, EpochKeys> = hkdf
        .expand(b"epoch keys")
        .expect("176 bytes is a valid length");
    keys.into_chunks()
}

/// Derive a cipher key for any cipher `C`, generic over both the digest and
/// the cipher. The output size is simply `C::KeySize`.
fn cipher_for<D: Digest, C: StreamCipher>(hkdf: &Hkdf<D>, nonce: &Array<u8, C::NonceSize>) -> C {
    let key = hkdf
        .expand::<C::KeySize>(b"cipher key")
        .expect("cipher keys are short");
    C::new(&key, nonce)
}

fn main() {
    // The key schedule works with any digest.
    let (client, server) = session_keys::<FakeSha256>(b"shared secret");
    println!("client key: {:02x?}", client.key);
    println!("client iv:  {:02x?}", client.iv);
    println!("server key: {:02x?}", server.key);
    println!("server iv:  {:02x?}", server.iv);
    let (client_512, _) = session_keys::<FakeSha512>(b"shared secret");
    assert_ne!(client.key, client_512.key);

    // Each epoch has its own session keys, split without index arithmetic.
    for (epoch, keys) in epoch_keys::<FakeSha256>(b"shared secret")
        .into_iter()
        .enumerate()
    {
        let (client, _server) = keys.parts();
        let (key, _iv) = client.parts();
        println!("epoch {epoch} client key: {:02x?}", key);
    }

    // Derive the key of a generic cipher and encrypt in place.
    let hkdf = Hkdf::<FakeSha256>::extract(None, b"input key material");
    let nonce = Array::from([0; 12]);
    let mut msg = *b"attack at dawn";
    cipher_for::<_, FakeChaCha20>(&hkdf, &nonce).apply_keystream(&mut msg);
    cipher_for::<_, FakeChaCha20>(&hkdf, &nonce).apply_keystream(&mut msg);
    assert_eq!(&msg, b"attack at dawn");

    // The PRK of SHA-512 has 64 bytes. Some protocols split a digest output
    // in half. The digest declares a plain `Len<64>`, so the halves are made
    // available with a compile-time checked cast.
    let prk = Hkdf::<FakeSha512>::extract(None, b"ikm").prk;
    let (enc_key, mac_key) = prk.cast(same_len!(Len<64>, Sum<Len<32>, Len<32>>)).parts();
    println!("enc key:    {:02x?}", enc_key);
    println!("mac key:    {:02x?}", mac_key);

    // Too long outputs are rejected.
    assert!(hkdf.expand::<Len<{ 255 * 32 + 1 }>>(b"").is_err());
}
