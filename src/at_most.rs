//! Proofs that one size is at most as long as another.

use core::{fmt, marker::PhantomData};

use crate::{ArrayLen, SameLen};

/// Proof that the [`ArrayLen`] `A` is at most as long as the [`ArrayLen`] `B`.
///
/// An `AtMost` is required by [`Array::truncate`](crate::Array::truncate),
/// [`Array::prefix_ref`](crate::Array::prefix_ref),
/// [`Array::split_prefix`](crate::Array::split_prefix),
/// [`Array::pad_from`](crate::Array::pad_from) and friends, which take or
/// fill the first `A::USIZE` elements of an array of size `B`.
///
/// It is obtained in the same ways as a [`SameLen`], in order of preference:
///
/// 1. For concrete sizes, the [`at_most!`](crate::at_most!) macro. It fails at
///    compile time, already during `cargo check`, if `A` is longer than `B`.
/// 2. In generic code where the lengths depend on what the *caller*
///    instantiates, take an `AtMost` as a parameter, or store it in a struct
///    when it is constructed. This makes the requirement part of the signature,
///    and callers with concrete types create it with `at_most!`.
/// 3. In generic code where `A` is at most as long as `B` for every
///    instantiation the caller can choose, [`AtMost::checked`]. It is only
///    reported when the code is monomorphized.
/// 4. [`AtMost::try_new`], to handle a mismatch at runtime.
// Invariant: An `AtMost<A, B>` only exists if `A::USIZE <= B::USIZE`. Unsafe
// code relies on this, so every constructor must ensure it.
pub struct AtMost<A, B>(PhantomData<(A, B)>);

// Manual impls, because deriving them would add unnecessary bounds on `A`
// and `B`.
impl<A, B> Clone for AtMost<A, B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, B> Copy for AtMost<A, B> {}

impl<A, B> fmt::Debug for AtMost<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AtMost")
    }
}

impl<A: ArrayLen, B: ArrayLen> AtMost<A, B> {
    /// Returns a proof that `A` is at most as long as `B`, or `None` if it is
    /// longer.
    ///
    /// The check only involves constants and is optimized away. Prefer
    /// [`at_most!`](crate::at_most!) for concrete sizes, which turns a
    /// mismatch into a compile error.
    pub const fn try_new() -> Option<Self> {
        if A::USIZE <= B::USIZE {
            // Invariant: Checked above.
            Some(AtMost(PhantomData))
        } else {
            None
        }
    }

    /// Returns a proof that `A` is at most as long as `B`, and fails to
    /// compile when monomorphized with sizes for which it is longer.
    ///
    /// This is for generic code in which `A` is at most as long as `B` for
    /// every instantiation the caller can choose, but in which the compiler
    /// cannot see it:
    ///
    /// ```
    /// use const_array::{Array, ArrayLen, AtMost, Sum};
    ///
    /// fn tail<T, A: ArrayLen, B: ArrayLen>(a: &Array<T, Sum<A, B>>) -> &[T] {
    ///     a.split_prefix(AtMost::<A, _>::checked()).1
    /// }
    /// ```
    ///
    /// # Pitfall: not reported by `cargo check`
    ///
    /// As for [`SameLen::checked`], the check runs when the calling code is
    /// monomorphized. `cargo check` and rust-analyzer do not report a
    /// mismatch, only `cargo build` does. If the lengths depend on the
    /// caller's choice of types, take an `AtMost` parameter instead:
    ///
    /// ```compile_fail
    /// use const_array::{Array, ArrayLen, AtMost, Len};
    ///
    /// fn first_16<S: ArrayLen>(a: &Array<u8, S>) -> &Array<u8, Len<16>> {
    ///     a.prefix_ref(AtMost::checked())
    /// }
    ///
    /// let _ = first_16::<Len<8>>(&Array::default());
    /// ```
    ///
    /// The error points at the generic code, not at the code that chose the
    /// sizes. See [`SameLen::checked`] for how to move it to `cargo check`.
    pub const fn checked() -> Self {
        const {
            assert!(
                A::USIZE <= B::USIZE,
                "AtMost::checked: the first size is longer than the second"
            )
        };
        // Invariant: Checked above.
        AtMost(PhantomData)
    }

    /// If `A` is at most as long as `B` and `B` at most as long as `C`, then
    /// `A` is at most as long as `C`.
    pub const fn trans<C: ArrayLen>(self, _other: AtMost<B, C>) -> AtMost<A, C> {
        // Invariant: `A::USIZE <= B::USIZE <= C::USIZE`.
        AtMost(PhantomData)
    }
}

impl<A: ArrayLen> AtMost<A, A> {
    /// Every size is at most as long as itself.
    pub const fn refl() -> Self {
        // Invariant: `A::USIZE <= A::USIZE`.
        AtMost(PhantomData)
    }
}

impl<A: ArrayLen, B: ArrayLen> SameLen<A, B> {
    /// If `A` has the same length as `B`, it is at most as long as `B`.
    pub const fn at_most(self) -> AtMost<A, B> {
        // Invariant: By `SameLen`'s invariant, `A::USIZE == B::USIZE`.
        AtMost(PhantomData)
    }
}

/// Create an [`AtMost`] proof for two concrete [`ArrayLens`][`ArrayLen`].
///
/// Fails to compile if the first size is longer than the second, already
/// during `cargo check`:
///
/// ```
/// use const_array::{at_most, AtMost, Len, Sum};
///
/// let proof: AtMost<Len<4>, Sum<Len<2>, Len<4>>> = at_most!(Len<4>, Sum<Len<2>, Len<4>>);
/// ```
///
/// ```compile_fail
/// use const_array::{at_most, Len};
///
/// let proof = at_most!(Len<7>, Len<6>);
/// ```
///
/// Like [`same_len!`](crate::same_len!), the macro rejects generic parameters
/// of the surrounding item during `cargo check`, and accepts `Self` and its
/// associated types in an impl for a concrete type:
///
/// ```
/// use const_array::{at_most, ArrayLen, AtMost, Len};
///
/// trait Digest {
///     type OutputSize: ArrayLen;
///     type BlockSize: ArrayLen;
///     const OUTPUT_FITS_BLOCK: AtMost<Self::OutputSize, Self::BlockSize>;
/// }
///
/// struct Sha256;
///
/// impl Digest for Sha256 {
///     type OutputSize = Len<32>;
///     type BlockSize = Len<64>;
///     const OUTPUT_FITS_BLOCK: AtMost<Self::OutputSize, Self::BlockSize> =
///         at_most!(Self::OutputSize, Self::BlockSize);
/// }
/// ```
///
/// ```compile_fail
/// use const_array::{at_most, ArrayLen, AtMost, Len};
///
/// fn generic<S: ArrayLen>() -> AtMost<S, Len<64>> {
///     at_most!(S, Len<64>)
/// }
/// ```
#[macro_export]
macro_rules! at_most {
    ($a:ty, $b:ty $(,)?) => {
        // See `same_len!`. The array length underflows if the first size is
        // longer, which names both lengths in the error.
        const {
            let _ = [(); <$b as $crate::ArrayLen>::USIZE - <$a as $crate::ArrayLen>::USIZE];
            match $crate::AtMost::<$a, $b>::try_new() {
                ::core::option::Option::Some(proof) => proof,
                ::core::option::Option::None => {
                    ::core::panic!("at_most!: the first size is longer than the second")
                }
            }
        }
    };
}
