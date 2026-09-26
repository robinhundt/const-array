//! Proofs that two sizes have the same length.

use core::{fmt, marker::PhantomData};

use crate::{ArrayLen, Len, Prod, Sum};

/// Proof that the [`ArrayLens`][`ArrayLen`] `A` and `B` have the same length.
///
/// A `SameLen` is required by [`Array::cast`](crate::Array::cast) and friends
/// to reinterpret an array of one size as an array of another size with the
/// same length, e.g. `Len<6>` and `Sum<Len<2>, Len<4>>`.
///
/// Ways to obtain one, in order of preference:
///
/// 1. For concrete sizes, the [`same_len!`](crate::same_len!) macro. It fails
///    at compile time, already during `cargo check`, if the lengths differ.
/// 2. In generic code where the lengths depend on what the *caller*
///    instantiates, take a `SameLen` as a parameter. This makes the requirement
///    part of the signature, and callers with concrete types create it with
///    `same_len!`.
/// 3. In generic code where the lengths are equal because of how the sizes are
///    built, compose a proof from the [lemmas](#lemmas), such as
///    [`SameLen::sum_assoc`] or [`SameLen::sum`]. They can't fail, so they need
///    no check at all.
/// 4. In generic code where the lengths are equal for every instantiation the
///    caller can choose, but no lemma applies, [`SameLen::checked`]. A mismatch
///    is a bug in the code that calls it, and it is only reported when the code
///    is monomorphized.
/// 5. [`SameLen::try_new`], to handle a mismatch at runtime.
///
/// # Lemmas
///
/// The lemmas prove facts that hold for all sizes, e.g. that `Sum<A, B>` has
/// the same length as `Sum<B, A>`. Combined with [`SameLen::trans`],
/// [`SameLen::sum`] and [`SameLen::prod`], they turn a rearrangement of a
/// size into a proof that `cargo check` accepts, in generic code too:
///
/// ```
/// use const_array::{Array, ArrayLen, Len, Prod, SameLen, Sum};
///
/// // A header followed by `N + M` blocks, split into the header, the first
/// // `N` blocks and the remaining `M` blocks.
/// fn split_blocks<T, H: ArrayLen, N: ArrayLen, M: ArrayLen>(
///     buf: &Array<T, Sum<H, Prod<Sum<N, M>, Len<16>>>>,
/// ) -> (&Array<T, H>, &Array<T, Prod<N, Len<16>>>, &Array<T, Prod<M, Len<16>>>) {
///     let proof = SameLen::refl().sum(SameLen::distrib_right());
///     let (header, blocks) = buf.cast_ref(proof).split_ref();
///     let (first, rest) = blocks.split_ref();
///     (header, first, rest)
/// }
/// # type Buf = Sum<Len<1>, Prod<Sum<Len<1>, Len<1>>, Len<16>>>;
/// # let buf = Array::<u8, Buf>::from_fn(|i| i as u8);
/// # let (header, first, rest) = split_blocks(&buf);
/// # assert_eq!(header.as_slice(), &buf[..1]);
/// # assert_eq!(first.as_slice(), &buf[1..17]);
/// # assert_eq!(rest.as_slice(), &buf[17..]);
/// ```
///
/// Like every cast, a lemma only changes how the elements are grouped, never
/// their order. E.g. casting `Sum<A, B>` to `Sum<B, A>` with
/// [`SameLen::sum_comm`] does not swap the parts, but splits the same
/// elements after the first `B::USIZE` instead of the first `A::USIZE`.
// Invariant: A `SameLen<A, B>` only exists if `A::USIZE == B::USIZE`. Unsafe
// code relies on this, so every constructor must ensure it. A length that
// overflows `usize` is a compile error wherever it is evaluated, and unsafe
// code evaluates the lengths of both sizes before relying on the proof, so the
// lemmas only need to hold for lengths that don't overflow.
//
// The proof is invariant in `A` and `B`, so it can't be coerced to a proof
// for other sizes. All `ArrayLen`s are `'static` today, so this is purely
// defensive.
pub struct SameLen<A, B>(pub(crate) Invariant<A, B>);

/// A marker that is invariant in `A` and `B`.
pub(crate) type Invariant<A, B> = PhantomData<fn(A, B) -> (A, B)>;

// Manual impls, because deriving them would add unnecessary bounds on `A`
// and `B`.
impl<A, B> Clone for SameLen<A, B> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, B> Copy for SameLen<A, B> {}

impl<A, B> fmt::Debug for SameLen<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SameLen")
    }
}

impl<A: ArrayLen, B: ArrayLen> SameLen<A, B> {
    /// Returns a proof that `A` and `B` have the same length, or `None` if
    /// they do not.
    ///
    /// The check only involves constants and is optimized away. Prefer
    /// [`same_len!`](crate::same_len!) for concrete sizes, which turns a
    /// mismatch into a compile error.
    #[must_use]
    #[inline]
    pub const fn try_new() -> Option<Self> {
        if A::USIZE == B::USIZE {
            // Invariant: Checked above.
            Some(SameLen(PhantomData))
        } else {
            None
        }
    }

    /// Returns a proof that `A` and `B` have the same length, and fails to
    /// compile when monomorphized with sizes that do not.
    ///
    /// This is for generic code in which the lengths are equal for every
    /// instantiation the caller can choose, but in which the compiler cannot
    /// see it, and no [lemma](#lemmas) applies:
    ///
    /// ```
    /// use const_array::{Array, ArrayLen, Len, Prod, SameLen, Sum};
    ///
    /// fn stack<T, A: ArrayLen>(a: Array<T, Sum<A, A>>) -> Array<T, Prod<Len<2>, A>> {
    ///     a.cast(SameLen::checked())
    /// }
    /// # let a = Array::<u8, Sum<Len<2>, Len<2>>>::from_fn(|i| i as u8);
    /// # assert_eq!(stack(a).as_slice(), &[0, 1, 2, 3]);
    /// ```
    ///
    /// # Pitfall: not reported by `cargo check`
    ///
    /// The check runs when the calling code is monomorphized. `cargo check`
    /// and rust-analyzer do not report a mismatch, only `cargo build` does,
    /// and only for the instantiations that are actually built. If the
    /// lengths depend on the caller's choice of types, take a `SameLen`
    /// parameter instead, so a mismatch is a type error at the call site:
    ///
    /// ```compile_fail
    /// use const_array::{Array, ArrayLen, Len, SameLen};
    ///
    /// fn to_32<S: ArrayLen>(a: Array<u8, S>) -> Array<u8, Len<32>> {
    ///     a.cast(SameLen::checked())
    /// }
    ///
    /// let _ = to_32::<Len<31>>(Array::default());
    /// ```
    ///
    /// The error points at the generic code, not at the code that chose the
    /// sizes. A `const` item that uses the instantiation moves the check to
    /// `cargo check`:
    ///
    /// ```compile_fail
    /// use const_array::{ArrayLen, Len, SameLen, Sum};
    ///
    /// struct Wrapper<A, B>(A, B);
    ///
    /// impl<A: ArrayLen, B: ArrayLen> Wrapper<A, B> {
    ///     const SPLIT: SameLen<A, Sum<B, Len<4>>> = SameLen::checked();
    /// }
    ///
    /// // 16 != 8 + 4, reported by `cargo check`.
    /// const _: () = {
    ///     let _ = Wrapper::<Len<16>, Len<8>>::SPLIT;
    /// };
    /// ```
    #[must_use]
    #[inline]
    pub const fn checked() -> Self {
        const {
            assert!(
                A::USIZE == B::USIZE,
                "SameLen::checked: the sizes have different lengths"
            )
        };
        // Invariant: Checked above.
        SameLen(PhantomData)
    }

    /// If `A` has the same length as `B`, then `B` has the same length as `A`.
    ///
    /// Useful to cast an array back to its original size, so a single proof
    /// parameter suffices:
    ///
    /// ```
    /// use const_array::{Array, ArrayLen, SameLen, Len, Sum};
    ///
    /// fn swap_halves<S: ArrayLen>(
    ///     a: Array<u8, S>,
    ///     proof: SameLen<S, Sum<Len<32>, Len<32>>>,
    /// ) -> Array<u8, S> {
    ///     let (lo, hi) = a.cast(proof).parts();
    ///     hi.concat(lo).cast(proof.symm())
    /// }
    /// # let a = Array::<u8, Len<64>>::from_fn(|i| i as u8);
    /// # let swapped = swap_halves(a, const_array::same_len!(Len<64>, Sum<Len<32>, Len<32>>));
    /// # assert_eq!((&swapped[..32], &swapped[32..]), (&a[32..], &a[..32]));
    /// ```
    #[must_use]
    #[inline]
    pub const fn symm(self) -> SameLen<B, A> {
        // Invariant: Equality is symmetric.
        SameLen(PhantomData)
    }

    /// If `A` has the same length as `B` and `B` the same length as `C`, then
    /// `A` has the same length as `C`.
    #[must_use]
    #[inline]
    pub const fn trans<C: ArrayLen>(self, _other: SameLen<B, C>) -> SameLen<A, C> {
        // Invariant: Equality is transitive.
        SameLen(PhantomData)
    }

    /// If `A` has the same length as `B` and `C` the same length as `D`, then
    /// `Sum<A, C>` has the same length as `Sum<B, D>`.
    ///
    /// Use it with [`SameLen::refl`] to rearrange one part of a [`Sum`].
    #[must_use]
    #[inline]
    pub const fn sum<C: ArrayLen, D: ArrayLen>(
        self,
        _other: SameLen<C, D>,
    ) -> SameLen<Sum<A, C>, Sum<B, D>> {
        // Invariant: `A::USIZE + C::USIZE == B::USIZE + D::USIZE`.
        SameLen(PhantomData)
    }

    /// If `A` has the same length as `B` and `C` the same length as `D`, then
    /// `Prod<A, C>` has the same length as `Prod<B, D>`.
    #[must_use]
    #[inline]
    pub const fn prod<C: ArrayLen, D: ArrayLen>(
        self,
        _other: SameLen<C, D>,
    ) -> SameLen<Prod<A, C>, Prod<B, D>> {
        // Invariant: `A::USIZE * C::USIZE == B::USIZE * D::USIZE`.
        SameLen(PhantomData)
    }
}

impl<A: ArrayLen, B: ArrayLen> SameLen<Sum<A, B>, Sum<B, A>> {
    /// Lemma: `A + B` has the same length as `B + A`.
    ///
    /// A cast with it does not swap the parts. It splits the same elements
    /// after the first `B::USIZE` instead of the first `A::USIZE`.
    #[must_use]
    #[inline]
    pub const fn sum_comm() -> Self {
        // Invariant: Addition is commutative.
        SameLen(PhantomData)
    }
}

impl<A: ArrayLen, B: ArrayLen, C: ArrayLen> SameLen<Sum<Sum<A, B>, C>, Sum<A, Sum<B, C>>> {
    /// Lemma: `(A + B) + C` has the same length as `A + (B + C)`.
    ///
    /// Use [`SameLen::symm`] for the other direction.
    #[must_use]
    #[inline]
    pub const fn sum_assoc() -> Self {
        // Invariant: Addition is associative.
        SameLen(PhantomData)
    }
}

impl<A: ArrayLen> SameLen<Sum<Len<0>, A>, A> {
    /// Lemma: `0 + A` has the same length as `A`.
    #[must_use]
    #[inline]
    pub const fn sum_zero_left() -> Self {
        // Invariant: 0 is the identity of addition.
        SameLen(PhantomData)
    }
}

impl<A: ArrayLen> SameLen<Sum<A, Len<0>>, A> {
    /// Lemma: `A + 0` has the same length as `A`.
    #[must_use]
    #[inline]
    pub const fn sum_zero_right() -> Self {
        // Invariant: 0 is the identity of addition.
        SameLen(PhantomData)
    }
}

impl<A: ArrayLen, B: ArrayLen> SameLen<Prod<A, B>, Prod<B, A>> {
    /// Lemma: `A * B` has the same length as `B * A`.
    ///
    /// A cast with it does not transpose. It views the same elements as `B`
    /// chunks of `A` elements instead of `A` chunks of `B` elements.
    #[must_use]
    #[inline]
    pub const fn prod_comm() -> Self {
        // Invariant: Multiplication is commutative.
        SameLen(PhantomData)
    }
}

impl<A: ArrayLen, B: ArrayLen, C: ArrayLen> SameLen<Prod<Prod<A, B>, C>, Prod<A, Prod<B, C>>> {
    /// Lemma: `(A * B) * C` has the same length as `A * (B * C)`.
    ///
    /// Use [`SameLen::symm`] for the other direction.
    #[must_use]
    #[inline]
    pub const fn prod_assoc() -> Self {
        // Invariant: Multiplication is associative.
        SameLen(PhantomData)
    }
}

impl<A: ArrayLen> SameLen<Prod<Len<1>, A>, A> {
    /// Lemma: `1 * A` has the same length as `A`.
    #[must_use]
    #[inline]
    pub const fn prod_one_left() -> Self {
        // Invariant: 1 is the identity of multiplication.
        SameLen(PhantomData)
    }
}

impl<A: ArrayLen> SameLen<Prod<A, Len<1>>, A> {
    /// Lemma: `A * 1` has the same length as `A`.
    #[must_use]
    #[inline]
    pub const fn prod_one_right() -> Self {
        // Invariant: 1 is the identity of multiplication.
        SameLen(PhantomData)
    }
}

impl<A: ArrayLen, B: ArrayLen, C: ArrayLen>
    SameLen<Prod<A, Sum<B, C>>, Sum<Prod<A, B>, Prod<A, C>>>
{
    /// Lemma: `A * (B + C)` has the same length as `A * B + A * C`.
    #[must_use]
    #[inline]
    pub const fn distrib_left() -> Self {
        // Invariant: Multiplication distributes over addition.
        SameLen(PhantomData)
    }
}

impl<A: ArrayLen, B: ArrayLen, C: ArrayLen>
    SameLen<Prod<Sum<A, B>, C>, Sum<Prod<A, C>, Prod<B, C>>>
{
    /// Lemma: `(A + B) * C` has the same length as `A * C + B * C`.
    ///
    /// `A + B` chunks of `C` elements are `A` chunks followed by `B` chunks,
    /// e.g. to split a buffer of blocks into its first blocks and the rest.
    #[must_use]
    #[inline]
    pub const fn distrib_right() -> Self {
        // Invariant: Multiplication distributes over addition.
        SameLen(PhantomData)
    }
}

impl<A: ArrayLen> SameLen<A, A> {
    /// Every size has the same length as itself.
    ///
    /// Useful in generic code, where [`same_len!`](crate::same_len!) cannot be
    /// used, to pass a proof for sizes that are known to be the same type,
    /// e.g. through an associated type bound.
    #[must_use]
    #[inline]
    pub const fn refl() -> Self {
        // Invariant: `A::USIZE == A::USIZE`.
        SameLen(PhantomData)
    }
}

/// Create a [`SameLen`] proof for two concrete [`ArrayLens`][`ArrayLen`].
///
/// Fails to compile if the lengths differ, already during `cargo check`:
///
/// ```
/// use const_array::{same_len, SameLen, Len, Sum};
///
/// let proof: SameLen<Len<6>, Sum<Len<2>, Len<4>>> = same_len!(Len<6>, Sum<Len<2>, Len<4>>);
/// ```
///
/// ```compile_fail
/// use const_array::{same_len, Len};
///
/// let proof = same_len!(Len<6>, Len<7>);
/// ```
///
/// The sizes must be concrete, so that the check can run during `cargo check`.
/// The macro therefore rejects generic parameters of the surrounding item,
/// also during `cargo check`. Generic code should take a [`SameLen`] as a
/// parameter instead, or use [`SameLen::checked`] where a mismatch is
/// impossible:
///
/// ```compile_fail
/// use const_array::{same_len, ArrayLen, Len};
///
/// fn generic<S: ArrayLen>() {
///     let proof = same_len!(S, Len<6>);
/// }
/// ```
///
/// `Self` and its associated types can be used in an impl for a concrete type.
/// A common pattern is a flat public size with an associated `const` that
/// proves it matches the structured size used internally, checked once per
/// implementing type during `cargo check`:
///
/// ```
/// use const_array::{same_len, ArrayLen, Len, Prod, SameLen, Sum};
///
/// trait Params {
///     type K: ArrayLen;
///     /// The public, flat size of an encoded key.
///     type KeySize: ArrayLen;
///     const KEY_PARTS: SameLen<Self::KeySize, Sum<Prod<Self::K, Len<384>>, Len<32>>>;
/// }
///
/// struct Small;
///
/// impl Params for Small {
///     type K = Len<2>;
///     type KeySize = Len<800>;
///     const KEY_PARTS: SameLen<Self::KeySize, Sum<Prod<Self::K, Len<384>>, Len<32>>> =
///         same_len!(Self::KeySize, Sum<Prod<Self::K, Len<384>>, Len<32>>);
/// }
/// ```
///
/// In an impl that is generic itself, `Self` is generic too, and the macro
/// rejects it:
///
/// ```compile_fail
/// use const_array::{same_len, ArrayLen, Len, SameLen};
///
/// trait Flat {
///     type Size: ArrayLen;
///     const IS_32: SameLen<Self::Size, Len<32>>;
/// }
///
/// struct Wrapper<S>(S);
///
/// impl<S: ArrayLen> Flat for Wrapper<S> {
///     type Size = S;
///     const IS_32: SameLen<S, Len<32>> = same_len!(Self::Size, Len<32>);
/// }
/// ```
#[macro_export]
macro_rules! same_len {
    ($a:ty, $b:ty $(,)?) => {
        // An inline `const` (unlike a nested `const` item) can mention `Self`.
        // Array lengths can't depend on generic parameters, so the array
        // below rejects generic sizes during `cargo check`, and it is a type
        // error that names both lengths if they differ.
        const {
            let _: [(); <$a as $crate::ArrayLen>::USIZE] = [(); <$b as $crate::ArrayLen>::USIZE];
            match $crate::SameLen::<$a, $b>::try_new() {
                ::core::option::Option::Some(proof) => proof,
                ::core::option::Option::None => {
                    ::core::panic!("same_len!: the sizes have different lengths")
                }
            }
        }
    };
}
