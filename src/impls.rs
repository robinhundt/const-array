//! Standard library trait impls for [`Array`] that delegate to its slice.

use core::{
    borrow::{Borrow, BorrowMut},
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
};

use crate::{Array, ArrayLen, Len};

impl<T, S: ArrayLen> Deref for Array<T, S> {
    type Target = [T];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, S: ArrayLen> DerefMut for Array<T, S> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<T, S: ArrayLen> AsRef<[T]> for Array<T, S> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, S: ArrayLen> AsMut<[T]> for Array<T, S> {
    #[inline]
    fn as_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

// `Eq`, `Ord` and `Hash` delegate to the slice, as `Borrow` requires.
impl<T, S: ArrayLen> Borrow<[T]> for Array<T, S> {
    #[inline]
    fn borrow(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, S: ArrayLen> BorrowMut<[T]> for Array<T, S> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T: Default, S: ArrayLen> Default for Array<T, S> {
    #[inline]
    fn default() -> Self {
        Self::from_fn(|_| T::default())
    }
}

impl<T, const N: usize> From<[T; N]> for Array<T, Len<N>> {
    #[inline]
    fn from(arr: [T; N]) -> Self {
        Array::new(arr)
    }
}

impl<T, const N: usize> From<Array<T, Len<N>>> for [T; N] {
    #[inline]
    fn from(arr: Array<T, Len<N>>) -> Self {
        arr.into_array()
    }
}

impl<'a, T, const N: usize> From<&'a [T; N]> for &'a Array<T, Len<N>> {
    #[inline]
    fn from(arr: &'a [T; N]) -> Self {
        Array::from_ref(arr)
    }
}

impl<'a, T, const N: usize> From<&'a mut [T; N]> for &'a mut Array<T, Len<N>> {
    #[inline]
    fn from(arr: &'a mut [T; N]) -> Self {
        Array::from_mut(arr)
    }
}

impl<'a, T, const N: usize> From<&'a Array<T, Len<N>>> for &'a [T; N] {
    #[inline]
    fn from(arr: &'a Array<T, Len<N>>) -> Self {
        arr.as_array()
    }
}

impl<'a, T, const N: usize> From<&'a mut Array<T, Len<N>>> for &'a mut [T; N] {
    #[inline]
    fn from(arr: &'a mut Array<T, Len<N>>) -> Self {
        arr.as_mut_array()
    }
}

impl<T: fmt::Debug, S: ArrayLen> fmt::Debug for Array<T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_slice(), f)
    }
}

impl<T: PartialEq<U>, U, S: ArrayLen> PartialEq<Array<U, S>> for Array<T, S> {
    #[inline]
    fn eq(&self, other: &Array<U, S>) -> bool {
        self.as_slice() == other.as_slice()
    }
}

// Only for `Len<N>`, so that arrays of different lengths cannot be compared
// by accident. Other sizes can be compared with `as_slice`.
impl<T: PartialEq<U>, U, const N: usize> PartialEq<[U; N]> for Array<T, Len<N>> {
    #[inline]
    fn eq(&self, other: &[U; N]) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: PartialEq<U>, U, const N: usize> PartialEq<Array<U, Len<N>>> for [T; N] {
    #[inline]
    fn eq(&self, other: &Array<U, Len<N>>) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<T: Eq, S: ArrayLen> Eq for Array<T, S> {}

impl<T: PartialOrd, S: ArrayLen> PartialOrd for Array<T, S> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl<T: Ord, S: ArrayLen> Ord for Array<T, S> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl<T: Hash, S: ArrayLen> Hash for Array<T, S> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state);
    }
}
