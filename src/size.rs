//! Array lengths and the array types backing them.

use core::{
    cmp::Ordering,
    convert::Infallible,
    fmt::{self, Debug},
    hash::{Hash, Hasher},
    marker::PhantomData,
    mem::{self, MaybeUninit},
    ptr, slice,
};

use crate::sealed;

/// The storage of an array with elements of type `T`.
///
/// This is an implementation detail of [`ArrayLen::ArrayType`] and not part of
/// the public API: plain arrays `[T; LEN]` for [`Len`], [`Concat`] for [`Sum`]
/// and [`Repeat`] for [`Prod`].
///
/// # Safety
/// `Self` must be *laid out as* `[T; Self::LEN]`: it has the same size,
/// alignment and validity invariant, and consists of nothing but those
/// `Self::LEN` values of `T`. Values and references can then be reinterpreted
/// between `Self` and `[T; Self::LEN]` in both directions, and `Self` is
/// `Send`, `Sync`, etc. exactly when `T` is.
///
/// Unsafe code in this crate relies on this for every `T`. The trait is
/// sealed, so the only implementations are the ones in this module.
pub unsafe trait ArrayType<T>: sealed::Sealed + Sized {
    /// The length of the array.
    const LEN: usize;

    #[doc(hidden)]
    /// Clone the array.
    ///
    /// This method is a work-around so that we can have a `Clone`
    /// implementation that only has a `T: Clone` bound. It clones the elements
    /// flat rather than by cloning the nested arrays, which moves every clone
    /// into place once more. For `Copy` types, this compiles to a plain copy.
    #[inline]
    fn clone_array(&self) -> Self
    where
        T: Clone,
    {
        let flat = as_slice::<T, Self>(self);
        build(|i| flat[i].clone())
    }
}

/// Build an [`ArrayType`] whose element at index `i` is `f(i)`, calling `f`
/// exactly once for each index, in order.
///
/// The array is built in place, like with `core::array::from_fn`. Building
/// the nested array types by value instead copies every level, and the nested
/// loops keep the optimizer from removing checks in `f`.
#[inline]
pub(crate) fn build<T, A: ArrayType<T>, F: FnMut(usize) -> T>(mut f: F) -> A {
    match try_build(|i| Ok::<T, Infallible>(f(i))) {
        Ok(array) => array,
        Err(never) => match never {},
    }
}

/// Fallible version of [`build`]. It stops at the first error of `f`, drops
/// the elements built so far and returns the error.
#[inline]
pub(crate) fn try_build<T, E, A: ArrayType<T>, F: FnMut(usize) -> Result<T, E>>(
    mut f: F,
) -> Result<A, E> {
    /// Drops the first `init` elements at `base` if `f` panics or returns an
    /// error.
    struct Guard<T> {
        base: *mut T,
        init: usize,
    }

    impl<T> Drop for Guard<T> {
        #[inline]
        fn drop(&mut self) {
            // SAFETY: The first `init` elements were written and are owned by
            // the guard, as the array is never returned.
            unsafe { ptr::drop_in_place(ptr::slice_from_raw_parts_mut(self.base, self.init)) }
        }
    }

    let mut array = MaybeUninit::<A>::uninit();
    let mut guard = Guard {
        base: array.as_mut_ptr().cast::<T>(),
        init: 0,
    };
    for i in 0..A::LEN {
        let x = f(i)?;
        // SAFETY: By `A`'s invariant, it is laid out as `[T; A::LEN]`, so
        // index `i < A::LEN` is in bounds.
        unsafe { guard.base.add(i).write(x) };
        guard.init = i + 1;
    }
    mem::forget(guard);
    // SAFETY: All `A::LEN` elements, i.e. all of `A`, are initialized.
    Ok(unsafe { array.assume_init() })
}

/// View an [`ArrayType`] as a slice of its `A::LEN` elements.
#[inline]
pub(crate) const fn as_slice<T, A: ArrayType<T>>(a: &A) -> &[T] {
    // SAFETY: By `A`'s invariant, it is laid out as `[T; A::LEN]`. The slice
    // borrows from `a`.
    unsafe { slice::from_raw_parts(ptr::from_ref(a).cast(), A::LEN) }
}

/// View an [`ArrayType`] as a mutable slice of its `A::LEN` elements.
#[inline]
pub(crate) const fn as_mut_slice<T, A: ArrayType<T>>(a: &mut A) -> &mut [T] {
    // SAFETY: By `A`'s invariant, it is laid out as `[T; A::LEN]`. The slice
    // mutably borrows from `a`.
    unsafe { slice::from_raw_parts_mut(ptr::from_mut(a).cast(), A::LEN) }
}

// SAFETY: `Self` is `[T; Self::LEN]`.
unsafe impl<T, const N: usize> ArrayType<T> for [T; N] {
    const LEN: usize = N;

    // The standard library's `Clone`, which is specialized to a plain copy for
    // `Copy` types even without optimizations.
    #[inline]
    fn clone_array(&self) -> Self
    where
        T: Clone,
    {
        <[T; N] as Clone>::clone(self)
    }
}

/// Two arrays stored one after the other.
///
/// This backs [`Sum`] sizes. It is an implementation detail and not part of
/// the public API.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Concat<A, B>(pub(crate) A, pub(crate) B);

// SAFETY: By their invariants, `A` and `B` are laid out as `[T; A::LEN]` and
// `[T; B::LEN]`. Both have alignment `align_of::<T>()` and a size that is a
// multiple of it, so `repr(C)` places `B` directly after `A` without padding.
// `Concat` is thus laid out as `[T; A::LEN + B::LEN]` and holds only those
// values of `T`.
unsafe impl<T, A: ArrayType<T>, B: ArrayType<T>> ArrayType<T> for Concat<A, B> {
    const LEN: usize = A::LEN + B::LEN;
}

/// An array `O` of arrays `I`, flattened into a single array.
///
/// This backs [`Prod`] sizes. `I` is only a parameter so that the element
/// type of `O` can be named. It is an implementation detail and not part of
/// the public API.
#[repr(transparent)]
pub struct Repeat<I, O>(O, PhantomData<I>);

// Manual impls, because deriving them would add unnecessary bounds on `I`.
impl<I, O: Clone> Clone for Repeat<I, O> {
    #[inline]
    fn clone(&self) -> Self {
        Repeat(self.0.clone(), PhantomData)
    }
}

impl<I, O: Copy> Copy for Repeat<I, O> {}

// SAFETY: By their invariants, `O` is laid out as `[I; O::LEN]` and `I` as
// `[T; I::LEN]`. `Repeat` is `repr(transparent)` over `O`, so it is laid out as
// `[[T; I::LEN]; O::LEN]`, which is `[T; I::LEN * O::LEN]`, and holds only
// those values of `T`.
unsafe impl<T, I: ArrayType<T>, O: ArrayType<I>> ArrayType<T> for Repeat<I, O> {
    const LEN: usize = I::LEN * O::LEN;
}

/// Trait for valid [`Array`](crate::Array) lengths.
///
/// This is either a plain length [`Len`], a sum of lengths [`Sum`] or a
/// product of lengths [`Prod`].
///
/// Every `ArrayLen` implements the standard traits `Copy`, `Debug`, `Default`,
/// `Eq`, `Ord` and `Hash`, and is `Send + Sync + 'static`. So `#[derive]`s on
/// types that are generic over an `ArrayLen` work without extra bounds:
///
/// ```
/// use const_array::{Array, ArrayLen, Len, Sum};
///
/// #[derive(Clone, Debug, Default, PartialEq)]
/// struct Key<S: ArrayLen> {
///     bytes: Array<u8, S>,
///     size: S,
/// }
///
/// let key = Key::<Sum<Len<16>, Len<16>>>::default();
/// assert_eq!(key.clone(), key);
/// ```
///
/// # Safety
/// For every `T`, `<Self::ArrayType<T> as ArrayType<T>>::LEN == Self::USIZE`.
/// With the `ArrayType` invariant, `Self::ArrayType<T>` is thus laid out as
/// `[T; Self::USIZE]` for every `T`.
pub unsafe trait ArrayLen:
    sealed::Sealed + Copy + Debug + Default + Eq + Ord + Hash + Send + Sync + 'static
{
    /// The number of elements.
    const USIZE: usize;

    /// The array type for this length. It has `Self::USIZE` elements.
    ///
    /// Its concrete type is an implementation detail. It only appears in
    /// bounds such as `S: ArrayLen<ArrayType<T>: Copy>`, see the `Copy` impl
    /// of [`Array`](crate::Array).
    type ArrayType<T>: ArrayType<T>;
}

/// A plain length of `N` elements.
#[derive(Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Len<const N: usize>;

/// Formats as `Len<N>`.
impl<const N: usize> Debug for Len<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Len<{N}>")
    }
}
/// An `A` followed by a `B`.
///
/// An [`Array`](crate::Array) of this size can be split into its parts with
/// [`Array::split_ref`](crate::Array::split_ref) and friends.
pub struct Sum<A, B>(PhantomData<(A, B)>);
/// `A` chunks of `B` elements.
///
/// An [`Array`](crate::Array) of this size can be viewed as an array of chunks
/// with [`Array::as_chunks`](crate::Array::as_chunks) and friends.
pub struct Prod<A, B>(PhantomData<(A, B)>);

// The following traits are implemented manually for `Sum` and `Prod`, because
// deriving them would add unnecessary bounds on `A` and `B` (`Debug` only
// needs its bounds to print the structure). `ArrayLen`
// requires them, so that `#[derive]`s on user types that are generic over an
// `ArrayLen` work.
impl<A, B> Clone for Sum<A, B> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, B> Copy for Sum<A, B> {}

/// Formats the structure of the size, e.g. `Sum<Len<2>, Len<4>>`.
impl<A: Debug + Default, B: Debug + Default> Debug for Sum<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Sum<{:?}, {:?}>", A::default(), B::default())
    }
}

impl<A, B> PartialEq for Sum<A, B> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<A, B> Eq for Sum<A, B> {}

impl<A, B> Hash for Sum<A, B> {
    #[inline]
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<A, B> Default for Sum<A, B> {
    #[inline]
    fn default() -> Self {
        Sum(PhantomData)
    }
}

impl<A, B> PartialOrd for Sum<A, B> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<A, B> Ord for Sum<A, B> {
    #[inline]
    fn cmp(&self, _other: &Self) -> Ordering {
        Ordering::Equal
    }
}

impl<A, B> Clone for Prod<A, B> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, B> Copy for Prod<A, B> {}

/// Formats the structure of the size, e.g. `Prod<Len<2>, Len<4>>`.
impl<A: Debug + Default, B: Debug + Default> Debug for Prod<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Prod<{:?}, {:?}>", A::default(), B::default())
    }
}

impl<A, B> PartialEq for Prod<A, B> {
    #[inline]
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<A, B> Eq for Prod<A, B> {}

impl<A, B> Hash for Prod<A, B> {
    #[inline]
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<A, B> Default for Prod<A, B> {
    #[inline]
    fn default() -> Self {
        Prod(PhantomData)
    }
}

impl<A, B> PartialOrd for Prod<A, B> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<A, B> Ord for Prod<A, B> {
    #[inline]
    fn cmp(&self, _other: &Self) -> Ordering {
        Ordering::Equal
    }
}

// SAFETY: `[T; N]::LEN` is `N`.
unsafe impl<const N: usize> ArrayLen for Len<N> {
    const USIZE: usize = N;

    type ArrayType<T> = [T; N];
}

// SAFETY: `Concat::LEN` is `A::ArrayType<T>::LEN + B::ArrayType<T>::LEN`, which
// is `A::USIZE + B::USIZE` by their invariants.
unsafe impl<A: ArrayLen, B: ArrayLen> ArrayLen for Sum<A, B> {
    const USIZE: usize = A::USIZE + B::USIZE;

    type ArrayType<T> = Concat<A::ArrayType<T>, B::ArrayType<T>>;
}

// SAFETY: `Repeat::LEN` is `I::LEN * O::LEN` with `I = B::ArrayType<T>` and
// `O = A::ArrayType<I>`. By their invariants, these are `B::USIZE` and
// `A::USIZE`.
unsafe impl<A: ArrayLen, B: ArrayLen> ArrayLen for Prod<A, B> {
    const USIZE: usize = A::USIZE * B::USIZE;

    type ArrayType<T> = Repeat<B::ArrayType<T>, A::ArrayType<B::ArrayType<T>>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    // `Array::clone` clones the elements with `clone_array`, so `Repeat::clone`
    // is only called directly.
    #[test]
    fn repeat_clone_clones_the_storage() {
        let r: Repeat<[u8; 2], [[u8; 2]; 3]> = Repeat([[1, 2], [3, 4], [5, 6]], PhantomData);
        assert_eq!(Clone::clone(&r).0, r.0);
    }
}
