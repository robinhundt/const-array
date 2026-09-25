# Examples

These examples sketch cryptographic APIs built on `Array`. The primitives in
[`crypto/fake.rs`](crypto/fake.rs) are **insecure** stand-ins that only have
the sizes of the real primitives. The traits they implement (`Digest`, `Mac`,
`StreamCipher`, `Aead`, `Kem`) are in [`crypto/traits.rs`](crypto/traits.rs).
Run an example with `cargo run --example <name>`.

| Example | Construction |
|---|---|
| [`hybrid_kem`](hybrid_kem.rs) | a KEM combining any two KEMs |
| [`aead`](aead.rs) | encrypt-then-MAC over any cipher and MAC, per-record nonces |
| [`hkdf`](hkdf.rs) | HKDF over any digest, deriving whole key schedules at once |
| [`lamport`](lamport.rs) | Lamport signatures over any digest |
| [`signature`](signature.rs) | signatures and hash-then-sign over any digest |
| [`truncated`](truncated.rs) | truncated digests, usable with HMAC |

## Patterns

### Sizes are associated types, generic sizes are `Sum`s and `Prod`s

A trait declares each size as an `ArrayLen`. A generic construction declares
its sizes in terms of its parts, with no bounds such as typenum's `Add`:

```rust
impl<K1: Kem, K2: Kem, D: Digest> Kem for Hybrid<K1, K2, D> {
    type EncapsulationKeySize = Sum<K1::EncapsulationKeySize, K2::EncapsulationKeySize>;
    // ...
}
```

Because the structure is in the type, `split_ref`, `parts`, `concat` and
`as_chunks` can't go out of bounds and need no length checks.
See [`hybrid_kem`](hybrid_kem.rs), [`aead`](aead.rs) (`EncryptThenMac`) and
[`lamport`](lamport.rs) (`Prod` for nested sizes).

### Concrete primitives expose flat sizes and cast internally

A concrete primitive declares `type SignatureSize = Len<64>`, even if it is
built from two 32 byte halves. Internally it bridges the structures with a
`same_len!` cast, which `cargo check` verifies:

```rust
let (r, s) = sig.cast_ref(same_len!(Len<64>, Sum<Len<32>, Len<32>>)).split_ref();
```

See [`signature`](signature.rs) and [`aead`](aead.rs) (nonce construction).

### Let the caller pick the output size

A method generic over its output size replaces a slice API with a runtime
length. The size is usually inferred, and one call can derive several keys
that are then split with `parts`:

```rust
type Direction = Sum<AesKey, Iv>;
let keys: Array<u8, Sum<Direction, Direction>> = hkdf.expand(b"session keys")?;
let (client, server) = keys.parts();
```

See [`hkdf`](hkdf.rs) and `Aead::open` in [`crypto/traits.rs`](crypto/traits.rs).

### Requiring sizes to match

Choose by *who* decides whether a requirement holds:

- **The same type** is needed, e.g. a signature scheme's prehash must be the
  digest output: an associated type bound, `S: PrehashSignature<PrehashSize =
  D::OutputSize>`. See [`signature`](signature.rs).
- **The caller** picks the types: take a `SameLen` or `AtMost` proof
  parameter. Callers with concrete types create it with `same_len!`, others
  with `try_new` at runtime. See `expand_seed` in [`signature`](signature.rs).
- **Each implementation** of a trait must satisfy it: an associated `const`
  proof, see below.

### Trait requirements as associated `const` proofs

A requirement on an implementation, e.g. "the digest output fits into a
block", becomes a `const` on the trait instead of a bound on every generic
signature:

```rust
trait Digest {
    type OutputSize: ArrayLen;
    type BlockSize: ArrayLen;
    const OUTPUT_FITS_BLOCK: AtMost<Self::OutputSize, Self::BlockSize>;
}
```

- **Concrete impls** prove it with the macros, over `Self`'s sizes. A
  violation is reported by `cargo check`, at the impl:
  `const FITS_OUTPUT: AtMost<Len<32>, Self::OutputSize> = at_most!(Len<32>, Self::OutputSize);`
- **Generic impls** compose the proofs of their parts, which can't fail:
  `const OUTPUT_FITS_BLOCK: AtMost<N, D::BlockSize> = D::FITS_OUTPUT.trans(D::OUTPUT_FITS_BLOCK);`
- **Consumers** use the proof wherever an operation requires one, e.g.
  `Array::pad_from(D::digest(key), D::OUTPUT_FITS_BLOCK, 0)` in HMAC.

Only impls whose sizes are generic parameters, such as `FakeDigest<OUT,
BLOCK>`, need `AtMost::checked()`. It is only reported by `cargo build`.

See [`truncated`](truncated.rs), `Digest` in
[`crypto/traits.rs`](crypto/traits.rs) and [`crypto/hmac.rs`](crypto/hmac.rs).
