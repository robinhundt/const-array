//! Iteration over [`Array`]s.

use core::{fmt, iter::FusedIterator, mem::MaybeUninit, ops::Range, ptr, slice};

use crate::{Array, ArrayLen, array::transmute_layout};

impl<'a, T, S: ArrayLen> IntoIterator for &'a Array<T, S> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, T, S: ArrayLen> IntoIterator for &'a mut Array<T, S> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}

impl<T, S: ArrayLen> IntoIterator for Array<T, S> {
    type Item = T;
    type IntoIter = IntoIter<T, S>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        const { Array::<T, S>::LAYOUT_OK };
        const { Array::<MaybeUninit<T>, S>::LAYOUT_OK };
        IntoIter {
            // SAFETY: Both arrays are laid out as `[T; S::USIZE]`, as
            // `MaybeUninit<T>` is laid out as `T`, and every `T` is a valid
            // `MaybeUninit<T>`. The iterator takes ownership of the elements.
            data: unsafe { transmute_layout(self) },
            alive: 0..S::USIZE,
        }
    }
}

/// A by-value iterator over the elements of an [`Array`].
pub struct IntoIter<T, S: ArrayLen> {
    /// The storage the elements are moved out of.
    data: Array<MaybeUninit<T>, S>,
    /// Invariant: `alive` is a subrange of `0..S::USIZE`. The elements of
    /// `data` at these indices are initialized and owned by the iterator. The
    /// others are not, and must not be read.
    alive: Range<usize>,
}

impl<T, S: ArrayLen> IntoIter<T, S> {
    /// The elements that have not been yielded yet.
    #[must_use]
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        let alive = &self.data[self.alive.clone()];
        // SAFETY: By the invariant, the elements at `alive` are initialized,
        // and `MaybeUninit<T>` is laid out as `T`. The slice borrows from
        // `self`.
        unsafe { slice::from_raw_parts(alive.as_ptr().cast::<T>(), alive.len()) }
    }

    /// The elements that have not been yielded yet, as a mutable slice.
    #[must_use]
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let alive = &mut self.data[self.alive.clone()];
        // SAFETY: By the invariant, the elements at `alive` are initialized,
        // and `MaybeUninit<T>` is laid out as `T`. The slice mutably borrows
        // from `self`.
        unsafe { slice::from_raw_parts_mut(alive.as_mut_ptr().cast::<T>(), alive.len()) }
    }

    /// Drop the elements at `range`.
    ///
    /// # Safety
    /// The elements at `range` must be initialized and no longer be in
    /// `alive`, so the iterator owns them but will not access them again.
    #[inline]
    unsafe fn drop_range(&mut self, range: Range<usize>) {
        // By the invariant, `range` is in bounds. `get_mut` rather than
        // indexing, because the optimizer can't always remove the bounds
        // check, and leaking the elements is better than a panic path.
        let Some(dead) = self.data.get_mut(range) else {
            return;
        };
        // SAFETY: By the caller, the elements are initialized and owned by the
        // iterator, which never accesses them again. If one of them panics on
        // drop, the others are still dropped.
        unsafe {
            ptr::drop_in_place(ptr::slice_from_raw_parts_mut(
                dead.as_mut_ptr().cast::<T>(),
                dead.len(),
            ))
        }
    }
}

impl<T, S: ArrayLen> Iterator for IntoIter<T, S> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        let i = self.alive.next()?;
        // SAFETY: `i` was in `alive`, so the element is initialized. `i` is no
        // longer in `alive`, so the caller takes ownership of it.
        Some(unsafe { self.data[i].assume_init_read() })
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<T> {
        let start = self.alive.start;
        let skipped = n.min(self.alive.len());
        self.alive.start += skipped;
        // SAFETY: The skipped elements were in `alive` and no longer are.
        unsafe { self.drop_range(start..start + skipped) };
        self.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.alive.size_hint()
    }

    #[inline]
    fn count(self) -> usize {
        self.alive.len()
    }

    #[inline]
    fn last(mut self) -> Option<T> {
        self.next_back()
    }
}

impl<T, S: ArrayLen> DoubleEndedIterator for IntoIter<T, S> {
    #[inline]
    fn next_back(&mut self) -> Option<T> {
        let i = self.alive.next_back()?;
        // SAFETY: As in `next`.
        Some(unsafe { self.data[i].assume_init_read() })
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<T> {
        let end = self.alive.end;
        let skipped = n.min(self.alive.len());
        self.alive.end -= skipped;
        // SAFETY: The skipped elements were in `alive` and no longer are.
        unsafe { self.drop_range(end - skipped..end) };
        self.next_back()
    }
}

impl<T, S: ArrayLen> ExactSizeIterator for IntoIter<T, S> {}

impl<T, S: ArrayLen> FusedIterator for IntoIter<T, S> {}

impl<T: Clone, S: ArrayLen> Clone for IntoIter<T, S> {
    #[inline]
    fn clone(&self) -> Self {
        let start = self.alive.start;
        // Grow `alive` as the clones are written, so that they are dropped if
        // a later `clone` panics.
        let mut new = IntoIter {
            data: Array::from_fn(|_| MaybeUninit::uninit()),
            alive: start..start,
        };
        for x in self.as_slice() {
            new.data[new.alive.end].write(x.clone());
            new.alive.end += 1;
        }
        new
    }
}

impl<T, S: ArrayLen> Drop for IntoIter<T, S> {
    #[inline]
    fn drop(&mut self) {
        let alive = self.alive.clone();
        self.alive = 0..0;
        // SAFETY: The elements were in `alive` and no longer are.
        unsafe { self.drop_range(alive) }
    }
}

impl<T: fmt::Debug, S: ArrayLen> fmt::Debug for IntoIter<T, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IntoIter").field(&self.as_slice()).finish()
    }
}
