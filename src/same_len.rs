//! Proofs that two sizes have the same length.

use core::{fmt, marker::PhantomData};

use crate::ArrayLen;

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
/// 3. In generic code where the lengths are equal for every instantiation the
///    caller can choose, [`SameLen::checked`]. A mismatch is a bug in the code
///    that calls it, and it is only reported when the code is monomorphized.
/// 4. [`SameLen::try_new`], to handle a mismatch at runtime.
// Invariant: A `SameLen<A, B>` only exists if `A::USIZE == B::USIZE`. Unsafe
// code relies on this, so every constructor must ensure it.
pub struct SameLen<A, B>(PhantomData<(A, B)>);

// Manual impls, because deriving them would add unnecessary bounds on `A`
// and `B`.
impl<A, B> Clone for SameLen<A, B> {
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
    /// see it, e.g. reordering the parts of a [`Sum`](crate::Sum):
    ///
    /// ```
    /// use const_array::{Array, ArrayLen, SameLen, Sum};
    ///
    /// fn rotate<T, A: ArrayLen, B: ArrayLen, C: ArrayLen>(
    ///     a: Array<T, Sum<Sum<A, B>, C>>,
    /// ) -> Array<T, Sum<A, Sum<B, C>>> {
    ///     a.cast(SameLen::checked())
    /// }
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
    ///     hi.concat_with(lo, proof.symm())
    /// }
    /// ```
    pub const fn symm(self) -> SameLen<B, A> {
        // Invariant: Equality is symmetric.
        SameLen(PhantomData)
    }
}

impl<A: ArrayLen> SameLen<A, A> {
    /// Every size has the same length as itself.
    ///
    /// Useful in generic code, where [`same_len!`](crate::same_len!) cannot be
    /// used, to pass a proof for sizes that are known to be the same type,
    /// e.g. through an associated type bound.
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
