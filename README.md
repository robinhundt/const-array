# Const Array

[![crates.io](https://img.shields.io/crates/v/const-array.svg)](https://crates.io/crates/const-array)
[![docs.rs](https://docs.rs/const-array/badge.svg)](https://docs.rs/const-array)

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

// Concatenating the parts gives back the structured size.
let reset: Array<u8, Iv> = nonce.concat(Array::from([0; 4]));
assert_eq!(reset[12], 0);

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

When a generic algorithm is parameterized by a trait, each implementation
states its flat size and proves that it matches the internal structure in an
associated `const SameLen<Self::FlatSize, Structured>`, created with
`same_len!`. `cargo check` then reports a mismatch for each concrete impl. A
default of `SameLen::checked()` in the trait saves the proof per impl, but a
mismatch is then only reported by `cargo build`. Internally, pick the
structure of each size to match how values are built and split, so casts are
only needed at the public boundary.

**Generic constructions expose the `Sum` of their parts.** The key of an
encrypt-then-MAC AEAD over any cipher `C` and MAC `M` has the size
`Sum<C::KeySize, M::KeySize>`. It can't be flattened on stable Rust, and it
needs no bounds. Callers that instantiate the construction with concrete
types can flatten it with `same_len!`.

**Casts in generic code** depend on who decides whether the lengths match:

1. If the lengths depend on the types the *caller* chooses, take a
   `SameLen` proof as a parameter. The requirement is then part of the
   signature, and callers create the proof with `same_len!`.
2. If the lengths are equal because of how the sizes are built, e.g. when
   reordering the parts of a `Sum`, compose a proof from the lemmas such as
   `SameLen::sum_comm`, `SameLen::sum_assoc` or `SameLen::distrib_right`,
   combined with `trans`, `sum` and `prod`. They can't fail, so they need no
   check, and they work in generic code during `cargo check`:

   ```rust
   use const_array::{Array, ArrayLen, SameLen, Sum};

   // Split a key made of `A`, `B` and `C` into the first part and the rest.
   fn split_first<T, A: ArrayLen, B: ArrayLen, C: ArrayLen>(
       key: Array<T, Sum<Sum<A, B>, C>>,
   ) -> (Array<T, A>, Array<T, Sum<B, C>>) {
       key.cast(SameLen::sum_assoc()).parts()
   }
   ```

   Like every cast, a lemma only changes how the elements are grouped, never
   their order: casting `Sum<A, B>` to `Sum<B, A>` splits the same elements
   after the first `B::USIZE` instead of after the first `A::USIZE`.
3. If the lengths are equal for every instantiation but no lemma applies,
   use `Array::cast_checked` or `SameLen::checked`. A mismatch would be a bug
   in the generic code itself. It is reported when the code is monomorphized,
   so **not by `cargo check`**, only by `cargo build`.
4. To handle a mismatch at runtime, use `Array::try_cast` or
   `SameLen::try_new`.

**Comparisons** work the same way. An `AtMost<A, B>` proves that `A` is at
most as long as `B`, and `AtLeast<B, A>` is an alias for it. Create it with
`at_most!` or `at_least!`, compose it from lemmas such as
`AtMost::prefix_of_sum`, or use `AtMost::checked` or `AtMost::try_new`. It is
required by `truncate`, `pad_from` and the prefix and suffix methods, e.g. for
truncated MAC tags or keys padded to a block. A trait requirement such as
"the digest output fits into one block" can be an associated `const` proof on
each implementation, instead of a bound on every generic signature. The rest
next to a prefix or suffix is a slice, because subtraction can't be expressed
as a type in generic code. To get it as an `Array`, name its size `R` and cast
from `Len<N>` to `Sum<P, R>` with `same_len!`.

**Parameter traits.** If the caller picks the sizes but no argument can carry
a proof, e.g. in `Default::default()`, let the caller implement a trait that
names the sizes and requires the proof as an associated `const`. The impl is
concrete, so `cargo check` verifies the proof:

```rust
use const_array::{at_least, ArrayLen, AtLeast, Len};

trait ModeParams {
    type NonceSize: ArrayLen;
    const NONCE_NOT_EMPTY: AtLeast<Self::NonceSize, Len<1>>;
}

struct Nonce96;

impl ModeParams for Nonce96 {
    type NonceSize = Len<12>;
    const NONCE_NOT_EMPTY: AtLeast<Self::NonceSize, Len<1>> = at_least!(Self::NonceSize, Len<1>);
}
```

**Derives** work on types that are generic over a size: every `ArrayLen`
implements `Copy`, `Debug`, `Default`, `Eq`, `Ord` and `Hash`, so
`#[derive(Clone, Debug, Default, PartialEq)] struct Key<S: ArrayLen>`
needs no extra bounds.

**Variance.** `Array<T, S>` is *invariant* in `T`, because its storage is
the associated type `S::ArrayType<T>`. `generic-array` and `hybrid-array`
have the same limitation. So unlike a `[&'static str; 2]`, an
`Array<&'static str, Len<2>>` can't be passed where an
`Array<&'a str, Len<2>>` is expected. Convert explicitly, e.g. with
`a.map(|s| s)`, or give the elements the shorter lifetime from the start.

**Interop with `typenum`-based crates** such as `generic-array` or
`hybrid-array` goes through plain arrays at concrete sizes, e.g.
`let b: [u8; 32] = digest.finalize().into(); Array::new(b)`. An
`Array<T, Len<N>>` compares equal to a `[T; N]` directly.

## Minimum supported Rust version

The minimum supported Rust version is 1.85.

## Examples

The [`examples`](https://github.com/robinhundt/const-array/tree/main/examples) directory sketches how cryptographic APIs could
look when built on `Array`: traits for digests, MACs, stream ciphers, AEADs
and KEMs, and generic constructions over them (HMAC, HKDF, encrypt-then-MAC,
a hybrid KEM, hash-then-sign, Lamport signatures, truncated digests). The
primitives are insecure stand-ins that only have the right sizes. Run them with e.g.
`cargo run --example hybrid_kem`.
