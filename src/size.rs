//! Array lengths and the array types backing them.

use core::{
    array,
    cmp::Ordering,
    fmt::{self, Debug},
    hash::{Hash, Hasher},
    marker::PhantomData,
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

    /// Build a new array from the provided closure.
    ///
    /// The element at index `i` of the returned array is `f(offset + i)`.
    fn build<F: FnMut(usize) -> T>(f: F, offset: usize) -> Self;

    #[doc(hidden)]
    /// Clone the array.
    ///
    /// This method is a work-around so that we can have a `Clone`
    /// implementation that only has a `T: Clone` bound and which
    /// can make use of the std library specialization of Clone
    /// for Copy types.
    fn clone_array(&self) -> Self
    where
        T: Clone;
}

/// View an [`ArrayType`] as a slice of its `A::LEN` elements.
pub(crate) const fn as_slice<T, A: ArrayType<T>>(a: &A) -> &[T] {
    // SAFETY: By `A`'s invariant, it is laid out as `[T; A::LEN]`. The slice
    // borrows from `a`.
    unsafe { slice::from_raw_parts(ptr::from_ref(a).cast(), A::LEN) }
}

/// View an [`ArrayType`] as a mutable slice of its `A::LEN` elements.
pub(crate) const fn as_mut_slice<T, A: ArrayType<T>>(a: &mut A) -> &mut [T] {
    // SAFETY: By `A`'s invariant, it is laid out as `[T; A::LEN]`. The slice
    // mutably borrows from `a`.
    unsafe { slice::from_raw_parts_mut(ptr::from_mut(a).cast(), A::LEN) }
}

// SAFETY: `Self` is `[T; Self::LEN]`.
unsafe impl<T, const N: usize> ArrayType<T> for [T; N] {
    const LEN: usize = N;

    fn build<F: FnMut(usize) -> T>(mut f: F, offset: usize) -> Self {
        array::from_fn(|i| f(offset + i))
    }

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

    fn build<F: FnMut(usize) -> T>(mut f: F, offset: usize) -> Self {
        Concat(A::build(&mut f, offset), B::build(&mut f, offset + A::LEN))
    }

    fn clone_array(&self) -> Self
    where
        T: Clone,
    {
        Concat(self.0.clone_array(), self.1.clone_array())
    }
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

    fn build<F: FnMut(usize) -> T>(mut f: F, offset: usize) -> Self {
        Repeat(
            O::build(|j| I::build(&mut f, offset + j * I::LEN), 0),
            PhantomData,
        )
    }

    fn clone_array(&self) -> Self
    where
        T: Clone,
    {
        let outer = as_slice::<I, O>(&self.0);
        Repeat(O::build(|j| outer[j].clone_array(), 0), PhantomData)
    }
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Len<const N: usize>;
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
// deriving them would add unnecessary bounds on `A` and `B`. `ArrayLen`
// requires them, so that `#[derive]`s on user types that are generic over an
// `ArrayLen` work.
impl<A, B> Clone for Sum<A, B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, B> Copy for Sum<A, B> {}

impl<A, B> Debug for Sum<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Sum")
    }
}

impl<A, B> PartialEq for Sum<A, B> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<A, B> Eq for Sum<A, B> {}

impl<A, B> Hash for Sum<A, B> {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<A, B> Default for Sum<A, B> {
    fn default() -> Self {
        Sum(PhantomData)
    }
}

impl<A, B> PartialOrd for Sum<A, B> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<A, B> Ord for Sum<A, B> {
    fn cmp(&self, _other: &Self) -> Ordering {
        Ordering::Equal
    }
}

impl<A, B> Clone for Prod<A, B> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<A, B> Copy for Prod<A, B> {}

impl<A, B> Debug for Prod<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Prod")
    }
}

impl<A, B> PartialEq for Prod<A, B> {
    fn eq(&self, _other: &Self) -> bool {
        true
    }
}

impl<A, B> Eq for Prod<A, B> {}

impl<A, B> Hash for Prod<A, B> {
    fn hash<H: Hasher>(&self, _state: &mut H) {}
}

impl<A, B> Default for Prod<A, B> {
    fn default() -> Self {
        Prod(PhantomData)
    }
}

impl<A, B> PartialOrd for Prod<A, B> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<A, B> Ord for Prod<A, B> {
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
