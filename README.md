# Const Array

<div class="warning">

**Warning:** This crate is experimental and contains LLM generated unsafe code.
Do not use in production.

</div>

This library is an exploration of the design space for const generic arrays,
similar to [generic-array][ga] or [hybrid-array][ha]. The main difference
is that it does not use [typenum][tn] at all and relies solely on
const generics and some unsafe code.

Sizes are plain lengths `Len<N>`, sums of sizes `Sum<A, B>` or products of
sizes `Prod<A, B>` (`A` chunks of `B` elements). Declaring sizes with their
structure up front makes splitting, concatenating and chunking free and
checked by the type system. Sizes with the same length but a different
structure are bridged with a `SameLen` proof, which the `same_len!` macro
checks at compile time.

```rust
use const_array::{same_len, Array, Len, Prod, Sum};

type Iv = Sum<Len<12>, Len<4>>;

let iv: Array<u8, Iv> = Array::from_fn(|i| i as u8);
let (nonce, counter) = iv.split_ref();
assert_eq!(counter.as_slice(), &[12, 13, 14, 15]);

// Reinterpret the IV as a plain 16 byte array and mask it in place. Both
// arrays must have the same size, so no element can be skipped.
let mut block: Array<u8, Len<16>> = iv.cast(same_len!(Iv, Len<16>));
let mask = Array::from([0xff; 16]);
block.zip_mut_with(&mask, |b, m| *b ^= m);
assert_eq!(block[0], 0xff);

let parts = nonce.concat(Array::from([0; 4]));
let _: Array<u8, Iv> = parts;

// View 4 blocks of 16 bytes as an array of blocks.
let buf: Array<u8, Prod<Len<4>, Len<16>>> = Array::from_fn(|i| i as u8);
for (i, block) in buf.as_chunks().iter().enumerate() {
    assert_eq!(block[0], 16 * i as u8);
}
```

[ga]: https://crates.io/crates/generic-array
[ha]: https://crates.io/crates/hybrid-array
[tn]: https://crates.io/crates/typenum

## API conventions

Sizes with the same length but a different structure are different types:
`Len<64>`, `Sum<Len<32>, Len<32>>` and `Sum<Sum<Len<16>, Len<16>>, Len<32>>`
all hold 64 elements, but a bound like `KeySize = Len<64>` only matches the
first one. APIs built on `Array` should therefore follow these conventions.

**Concrete primitives expose flat sizes and cast internally.** A primitive
such as a signature scheme declares `type SignatureSize = Len<64>`, even if it
is made of two 32 byte scalars. Internally, it splits the signature with a
`same_len!` checked cast, which fails during `cargo check` if the lengths
differ. This keeps the internal structure out of the public API, so it can
change without breaking callers.

```rust
use const_array::{same_len, Array, Len, Sum};

type Rs = Sum<Len<32>, Len<32>>;

fn split_signature(sig: &Array<u8, Len<64>>) -> (&Array<u8, Len<32>>, &Array<u8, Len<32>>) {
    sig.cast_ref(same_len!(Len<64>, Rs)).split_ref()
}
```

When a generic algorithm is parameterized by a trait, state each flat size once
per implementation and put its proof in an associated `const` of type
`SameLen<Self::FlatSize, StructuredSize>`, created with
`same_len!(Self::FlatSize, StructuredSize)`. In an impl for a concrete type,
a mismatch is reported by `cargo check`. A default of `SameLen::checked()` in
the trait saves writing one proof per implementation, but a mismatch is only
reported by `cargo build`. Internally, pick the structure of each size to match how the
values are built and split, so casts are only needed at the public boundary.

**Generic constructions expose the `Sum` of their parts.** The key of an
encrypt-then-MAC AEAD over any cipher `C` and MAC `M` has the size
`Sum<C::KeySize, M::KeySize>`. It can't be flattened on stable Rust, and it
needs no bounds. Callers that instantiate the construction with concrete
types can flatten it with `same_len!`.

**Casts in generic code** depend on who decides whether the lengths match:

1. If the lengths depend on the types the *caller* chooses, take a
   `SameLen` proof as a parameter. The requirement is then part of the
   signature, and callers create the proof with `same_len!`.
2. If the lengths are equal for every instantiation, e.g. when reordering the
   parts of a `Sum`, use `Array::cast_checked` or `SameLen::checked`. A
   mismatch would be a bug in the generic code itself. It is reported when
   the code is monomorphized, so **not by `cargo check`**, only by
   `cargo build`.
3. To handle a mismatch at runtime, use `Array::try_cast` or
   `SameLen::try_new`.

**Comparisons** work the same way with an `AtMost<A, B>` proof that `A` is
at most as long as `B`, created with `at_most!`, `AtMost::checked` or
`AtMost::try_new`. It is required by `truncate`, `prefix_ref`,
`split_prefix` and `pad_from`, e.g. for truncated MAC tags or keys padded to
a block. A requirement of a trait, such as "the digest output fits into one
block", can be an associated `const` proof that each implementation
provides, instead of a bound that every generic signature has to repeat.
Subtraction can't be expressed as a type in generic code, so the rest after
a prefix is a slice. If it is needed as an `Array`, the caller names its
size and bridges it with `same_len!`, e.g. from `Len<N>` to `Sum<P, R>`.

**Derives** work on types that are generic over a size: every `ArrayLen`
implements `Copy`, `Debug`, `Default`, `Eq`, `Ord` and `Hash`, so
`#[derive(Clone, Debug, Default, PartialEq)] struct Key<S: ArrayLen>`
needs no extra bounds.

**Interop with `typenum`-based crates** such as `generic-array` or
`hybrid-array` goes through plain arrays at concrete sizes, e.g.
`let b: [u8; 32] = digest.finalize().into(); Array::new(b)`. An
`Array<T, Len<N>>` compares equal to a `[T; N]` directly.

## Examples

The [`examples`](https://github.com/robinhundt/const-array/tree/main/examples) directory sketches how cryptographic APIs could
look when built on `Array`: traits for digests, MACs, stream ciphers, AEADs
and KEMs, and generic constructions over them (HMAC, HKDF, encrypt-then-MAC,
a hybrid KEM, hash-then-sign, Lamport signatures). The primitives are
insecure stand-ins that only have the right sizes. Run them with e.g.
`cargo run --example hybrid_kem`.
