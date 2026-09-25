//! A hybrid KEM combining any two KEMs, generic over the combiner's digest.
//!
//! Shows:
//! - `Hybrid<K1, K2, D>`: its key and ciphertext sizes are `Sum`s of the
//!   component sizes, and its shared secret size is `D::OutputSize`. None of
//!   this needs type-level arithmetic bounds such as typenum's `Add`.
//! - splitting composite keys and ciphertexts with `split_ref`, which cannot go
//!   out of bounds.
//! - composing hybrids recursively: a hybrid of a hybrid is again a `Kem`.
//!
//! Run with `cargo run --example hybrid_kem`.

#[allow(dead_code)]
mod crypto;

use core::marker::PhantomData;

use const_array::{Array, ArrayLen, Sum};
use crypto::{
    fake::{FakeKem, FakeMlKem768, FakeRng, FakeSha256, FakeSha512, FakeX25519},
    traits::{Ciphertext, DecapsulationKey, Digest, EncapsulationKey, Kem, Rng, SharedSecret},
};

/// Hybrid of the KEMs `K1` and `K2`. The shared secret hashes both component
/// secrets together with the ciphertexts and the second encapsulation key,
/// similar to X-Wing.
pub struct Hybrid<K1, K2, D>(PhantomData<(K1, K2, D)>);

impl<K1: Kem, K2: Kem, D: Digest> Hybrid<K1, K2, D> {
    fn combine(
        ss1: &SharedSecret<K1>,
        ss2: &SharedSecret<K2>,
        ct1: &Ciphertext<K1>,
        ct2: &Ciphertext<K2>,
        ek2: &EncapsulationKey<K2>,
    ) -> SharedSecret<Self> {
        D::default()
            .chain(b"hybrid kem")
            .chain(ss1)
            .chain(ss2)
            .chain(ct1)
            .chain(ct2)
            .chain(ek2)
            .finalize()
    }
}

impl<K1: Kem, K2: Kem, D: Digest> Kem for Hybrid<K1, K2, D> {
    type EncapsulationKeySize = Sum<K1::EncapsulationKeySize, K2::EncapsulationKeySize>;
    type DecapsulationKeySize = Sum<
        Sum<K1::DecapsulationKeySize, K2::DecapsulationKeySize>,
        // The decapsulation key also stores the second encapsulation key,
        // as the combiner needs it.
        K2::EncapsulationKeySize,
    >;
    type CiphertextSize = Sum<K1::CiphertextSize, K2::CiphertextSize>;
    type SharedSecretSize = D::OutputSize;

    fn generate(rng: &mut impl Rng) -> (DecapsulationKey<Self>, EncapsulationKey<Self>) {
        let (dk1, ek1) = K1::generate(rng);
        let (dk2, ek2) = K2::generate(rng);
        let dk = Array::concat(Array::concat(dk1, dk2), ek2.clone());
        (dk, Array::concat(ek1, ek2))
    }

    fn encapsulate(
        ek: &EncapsulationKey<Self>,
        rng: &mut impl Rng,
    ) -> (Ciphertext<Self>, SharedSecret<Self>) {
        let (ek1, ek2) = ek.split_ref();
        let (ct1, ss1) = K1::encapsulate(ek1, rng);
        let (ct2, ss2) = K2::encapsulate(ek2, rng);
        let ss = Self::combine(&ss1, &ss2, &ct1, &ct2, ek2);
        (Array::concat(ct1, ct2), ss)
    }

    fn decapsulate(dk: &DecapsulationKey<Self>, ct: &Ciphertext<Self>) -> SharedSecret<Self> {
        let (dks, ek2) = dk.split_ref();
        let (dk1, dk2) = dks.split_ref();
        let (ct1, ct2) = ct.split_ref();
        let ss1 = K1::decapsulate(dk1, ct1);
        let ss2 = K2::decapsulate(dk2, ct2);
        Self::combine(&ss1, &ss2, ct1, ct2, ek2)
    }
}

/// Run a full key exchange with any KEM and print its sizes.
fn exchange<K: Kem>(name: &str, rng: &mut impl Rng) -> SharedSecret<K> {
    let (dk, ek) = K::generate(rng);
    let (ct, ss_sender) = K::encapsulate(&ek, rng);
    let ss_receiver = K::decapsulate(&dk, &ct);
    assert_eq!(ss_sender, ss_receiver);
    println!(
        "{name:<18} ek: {:>4} B, dk: {:>4} B, ct: {:>4} B, ss: {:>2} B",
        K::EncapsulationKeySize::USIZE,
        K::DecapsulationKeySize::USIZE,
        K::CiphertextSize::USIZE,
        K::SharedSecretSize::USIZE,
    );
    ss_sender
}

/// ML-KEM-768 combined with X25519, as in X-Wing.
type XWingish = Hybrid<FakeMlKem768, FakeX25519, FakeSha256>;

/// A hybrid of a hybrid: sizes nest automatically.
type Triple = Hybrid<XWingish, FakeKem<64, 64, 64>, FakeSha512>;

fn main() {
    let mut rng = FakeRng::default();
    exchange::<FakeMlKem768>("ML-KEM-768", &mut rng);
    exchange::<FakeX25519>("X25519", &mut rng);
    exchange::<XWingish>("X-Wing(ish)", &mut rng);
    let ss = exchange::<Triple>("Triple hybrid", &mut rng);
    println!("triple shared secret: {:02x?}", ss);

    // The size of a composite ciphertext is known in the type, so e.g. a
    // wire format can receive it into a fixed-size buffer.
    let wire = vec![0u8; <XWingish as Kem>::CiphertextSize::USIZE];
    let ct: &Ciphertext<XWingish> = wire.as_slice().try_into().expect("exact length");
    let (ml_kem_ct, x25519_ct) = ct.split_ref();
    assert_eq!((ml_kem_ct.len(), x25519_ct.len()), (1088, 32));
}
