//! HMAC (RFC 2104), generic over any [`Digest`].

use const_array::{Array, ArrayLen};

use super::traits::{Digest, Mac};

/// HMAC with the digest `D`.
#[derive(Clone)]
pub struct Hmac<D: Digest> {
    inner: D,
    outer: D,
}

impl<D: Digest> Hmac<D> {
    /// HMAC accepts keys of any length.
    pub fn new_from_slice(key: &[u8]) -> Self {
        // The key, zero-padded to the digest's `BlockSize`. Keys longer than
        // a block are hashed first. The hash fits into a block, as the
        // `Digest` trait requires.
        let block = if key.len() > D::BlockSize::USIZE {
            Array::pad_from(D::digest(key), D::OUTPUT_FITS_BLOCK, 0)
        } else {
            let mut block = Array::<u8, D::BlockSize>::default();
            block[..key.len()].copy_from_slice(key);
            block
        };
        Self {
            inner: D::default().chain(&block.map_ref(|b| b ^ 0x36)),
            outer: D::default().chain(&block.map_ref(|b| b ^ 0x5c)),
        }
    }
}

/// As a [`Mac`] with a fixed key size, HMAC uses keys of the digest's
/// output size, as recommended by RFC 2104.
impl<D: Digest> Mac for Hmac<D> {
    type KeySize = D::OutputSize;
    type TagSize = D::OutputSize;

    fn new(key: &Array<u8, D::OutputSize>) -> Self {
        Self::new_from_slice(key)
    }

    fn update(&mut self, data: &[u8]) {
        self.inner.update(data);
    }

    fn finalize(self) -> Array<u8, D::OutputSize> {
        let inner = self.inner.finalize();
        self.outer.chain(&inner).finalize()
    }
}
