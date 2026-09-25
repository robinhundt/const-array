//! Iteration over [`Array`]s.

use core::{fmt, iter::FusedIterator, mem::ManuallyDrop, ops::Range, ptr, slice};

use crate::{Array, ArrayLen};

impl<'a, T, S: ArrayLen> IntoIterator for &'a Array<T, S> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T, S: ArrayLen> IntoIterator for &'a mut Array<T, S> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T, S: ArrayLen> IntoIterator for Array<T, S> {
    type Item = T;
    type IntoIter = IntoIter<T, S>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            data: ManuallyDrop::new(self),
            alive: 0..S::USIZE,
        }
    }
}

/// A by-value iterator over the elements of an [`Array`].
pub struct IntoIter<T, S: ArrayLen> {
    /// The array the elements are moved out of.
    data: ManuallyDrop<Array<T, S>>,
    /// Invariant: `alive` is a subrange of `0..S::USIZE`. The iterator owns
    /// the elements of `data` at these indices. The others have been moved out
    /// and must not be accessed.
    alive: Range<usize>,
}

impl<T, S: ArrayLen> IntoIter<T, S> {
    /// Pointer to the first element of `data`, which is laid out as
    /// `[T; S::USIZE]`.
    fn base(&self) -> *const T {
        const { Array::<T, S>::LAYOUT_OK };
        ptr::from_ref(&*self.data).cast()
    }

    /// Mutable pointer to the first element of `data`, which is laid out as
    /// `[T; S::USIZE]`.
    fn base_mut(&mut self) -> *mut T {
        const { Array::<T, S>::LAYOUT_OK };
        ptr::from_mut(&mut *self.data).cast()
    }

    /// The elements that have not been yielded yet.
    pub fn as_slice(&self) -> &[T] {
        // SAFETY: By the invariant, the elements at `alive` are in bounds and
        // initialized. The slice borrows from `self`.
        unsafe { slice::from_raw_parts(self.base().add(self.alive.start), self.alive.len()) }
    }

    /// The elements that have not been yielded yet, as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let start = self.alive.start;
        let len = self.alive.len();
        // SAFETY: By the invariant, the elements at `alive` are in bounds and
        // initialized. The slice mutably borrows from `self`.
        unsafe { slice::from_raw_parts_mut(self.base_mut().add(start), len) }
    }
}

impl<T, S: ArrayLen> Iterator for IntoIter<T, S> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let i = self.alive.next()?;
        // SAFETY: `i` was in `alive`, so the element is in bounds and
        // initialized. `i` is no longer in `alive`, so the caller takes
        // ownership of it.
        Some(unsafe { ptr::read(self.base().add(i)) })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.alive.size_hint()
    }
}

impl<T, S: ArrayLen> DoubleEndedIterator for IntoIter<T, S> {
    fn next_back(&mut self) -> Option<T> {
        let i = self.alive.next_back()?;
        // SAFETY: As in `next`.
        Some(unsafe { ptr::read(self.base().add(i)) })
    }
}

impl<T, S: ArrayLen> ExactSizeIterator for IntoIter<T, S> {}

impl<T, S: ArrayLen> FusedIterator for IntoIter<T, S> {}

impl<T, S: ArrayLen> Drop for IntoIter<T, S> {
    fn drop(&mut self) {
        // SAFETY: The iterator owns the elements at `alive` and is not used
        // after `drop`. `data` is `ManuallyDrop`, so they are dropped
        // only here.
        unsafe { ptr::drop_in_place(self.as_mut_slice()) }
    }
}

impl<T: fmt::Debug, S: ArrayLen> fmt::Debug for IntoIter<T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoIter").field(&self.as_slice()).finish()
    }
}
