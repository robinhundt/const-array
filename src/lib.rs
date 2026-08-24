#![doc = include_str!("../README.md")]
#![no_std]
#![warn(clippy::undocumented_unsafe_blocks)]

use core::{
    array,
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
    ptr, slice,
};

/// Trait for arrays of type `T`.
///
/// This abstracts over plain arrays `[T; LEN]` and concatenations
/// of arrays [`Concat`].
///
/// # Safety
/// It must be sound to transmute `&Self` into `&[T; Self::LEN]` and
/// `&mut Self` into `&mut [T; Self::LEN]` and vice versa.
pub unsafe trait ArrayType<T>: sealed::Sealed + Sized {
    /// The length of the array.
    const LEN: usize;

    /// Build a new array from the provided closure.
    ///
    /// The element at index `i` of the returned array is `f(offset + i)`.
    fn build<F: FnMut(usize) -> T>(f: F, offset: usize) -> Self;
}

// SAFETY: Self is `[T; N]` and `Self::LEN = N` so it is trivially sound to
// transmute references of it to itself.
unsafe impl<T, const N: usize> ArrayType<T> for [T; N] {
    const LEN: usize = N;

    fn build<F: FnMut(usize) -> T>(mut f: F, offset: usize) -> Self {
        array::from_fn(|i| f(offset + i))
    }
}

/// Concatenation of two [`Arrays`][`Array`].
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Concat<A, B>(pub A, pub B);

// SAFETY: By `A` and `B`'s invariants, both have alignment `align_of::<T>()`
// and a size that is an exact multiple of `size_of::<T>()`. So `repr(C)`
// places `B` at offset `size_of::<A>()` with no gap and adds no tail padding,
// giving `Concat` the size and alignment of `[T; A::LEN + B::LEN]` with every
// slot holding an initialized `T`. That sum is `Self::LEN`, so the transmute is
// sound in both directions.
unsafe impl<T, A: ArrayType<T>, B: ArrayType<T>> ArrayType<T> for Concat<A, B> {
    const LEN: usize = A::LEN + B::LEN;

    fn build<F: FnMut(usize) -> T>(mut f: F, offset: usize) -> Self {
        Concat(A::build(&mut f, offset), B::build(&mut f, offset + A::LEN))
    }
}

/// Trait for valid [`Array`] sizes.
///
/// This is either a plain size [`U`] or a sum of sizes [`Sum`].
///
/// # Safety
/// For any type `T, S: ArraySize` it must hold that
/// `<S::ArrayType<T> as ArrayType<T>>::LEN == S::USIZE`.
pub unsafe trait ArraySize: sealed::Sealed {
    /// The value of the array size.
    const USIZE: usize;

    /// The array type for this size.
    ///
    /// It is guaranteed that for any `S: ArraySize` it holds that
    /// `<S::ArrayType<T> as ArrayType<T>>::LEN == S::USIZE`.
    type ArrayType<T>: ArrayType<T>;
}

/// A simple [`ArraySize`] over a const generic `N`.
pub struct U<const N: usize>;
/// The sum of two [`ArraySizes`][`ArraySize`].
pub struct Sum<A, B>(PhantomData<(A, B)>);

// SAFETY: The LEN of the ArrayType and the USIZE are both `N`.
unsafe impl<const N: usize> ArraySize for U<N> {
    const USIZE: usize = N;

    type ArrayType<T> = [T; N];
}

// SAFETY: The invariant holds for A and B. By setting `Self::USIZE` as the sum
// of the individual `USIZE` and using `Concat` which implements
// `const LEN: usize = A::LEN + B::LEN;` we know the invariant holds for the Sum
// impl.
unsafe impl<A: ArraySize, B: ArraySize> ArraySize for Sum<A, B> {
    const USIZE: usize = A::USIZE + B::USIZE;

    type ArrayType<T> = Concat<A::ArrayType<T>, B::ArrayType<T>>;
}

/// A generic array for a type `T` and an [`ArraySize`] `S`.
#[repr(transparent)]
pub struct Array<T, S: ArraySize>(S::ArrayType<T>);

impl<T: Clone, S: ArraySize<ArrayType<T>: Clone>> Clone for Array<T, S> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl<T: Copy, S: ArraySize<ArrayType<T>: Copy>> Copy for Array<T, S> {}

impl<T, S: ArraySize> Array<T, S> {
    // These asserts should never fail for valid implementations of ArrayType and
    // ArraySize. Since these traits are also sealed, these checks are only an
    // additional check for errors in this library and can be used for easier
    // reasoning of unsafe blocks.
    const LAYOUT_OK: () = {
        assert!(<S::ArrayType<T> as ArrayType<T>>::LEN == S::USIZE);
        assert!(mem::align_of::<Self>() == mem::align_of::<T>());
        assert!(mem::size_of::<Self>() == mem::size_of::<T>() * S::USIZE);
    };

    /// View the [`Array`] as a slice.
    pub fn as_slice(&self) -> &[T] {
        const { Self::LAYOUT_OK };
        // SAFETY:
        // - `LAYOUT_OK` has checked that `Array<T, S>` has the size and alignment of
        //   `[T; S::USIZE]`, and `Array` is `repr(transparent)` over `S::ArrayType<T>`,
        //   so the elements start at offset 0.
        // - By the `ArrayType` invariant each of those `S::USIZE` slots holds an
        //   initialized, valid `T`.
        // - The slice borrows from `&self` for `'_`, so it cannot outlive the array or
        //   alias a `&mut` to it.
        unsafe { slice::from_raw_parts(ptr::from_ref(self).cast(), S::USIZE) }
    }

    /// View the [`Array`] as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        const { Self::LAYOUT_OK };
        // SAFETY:
        // - `LAYOUT_OK` has checked that `Array<T, S>` has the size and alignment of
        //   `[T; S::USIZE]`, and `Array` is `repr(transparent)` over `S::ArrayType<T>`,
        //   so the elements start at offset 0.
        // - By the `ArrayType` invariant each of those `S::USIZE` slots holds an
        //   initialized, valid `T`.
        // - The slice derives from `&mut self`, so it is the only live access to the
        //   storage for `'_`.
        unsafe { slice::from_raw_parts_mut(ptr::from_mut(self).cast(), S::USIZE) }
    }

    /// Construct a new array from a function.
    ///
    /// The function is called with each index of the array in order.
    pub fn from_fn<F: FnMut(usize) -> T>(f: F) -> Array<T, S> {
        Array(S::ArrayType::build(f, 0))
    }
}

impl<T, A: ArraySize, B: ArraySize> Array<T, Sum<A, B>> {
    /// Split a concatenated [`Array`] into its parts.
    pub fn parts(self) -> (Array<T, A>, Array<T, B>) {
        let Concat(a, b) = self.0;
        (Array(a), Array(b))
    }
}

impl<T, S: ArraySize> Deref for Array<T, S> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<T, S: ArraySize> DerefMut for Array<T, S> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<T, S: ArraySize> AsRef<[T]> for Array<T, S> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T, S: ArraySize> AsMut<[T]> for Array<T, S> {
    fn as_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T: Default, S: ArraySize> Default for Array<T, S> {
    fn default() -> Self {
        Self::from_fn(|_| T::default())
    }
}

impl<T, const N: usize> From<[T; N]> for Array<T, U<N>> {
    fn from(arr: [T; N]) -> Self {
        Array(arr)
    }
}

/// Error when converting a slice into an [`Array`].
#[derive(Debug, Copy, Clone)]
pub struct TryFromSliceError(());

impl<T, S: ArraySize> TryFrom<&[T]> for &Array<T, S> {
    type Error = TryFromSliceError;

    fn try_from(slice: &[T]) -> Result<Self, Self::Error> {
        const { <Array<T, S>>::LAYOUT_OK };
        if slice.len() == S::USIZE {
            let ptr: *const Array<T, S> = slice.as_ptr().cast();
            // SAFETY:
            // - `slice.len() == S::USIZE`, and `LAYOUT_OK` gives `Array<T, S>` the size and
            //   alignment of `[T; S::USIZE]`, so `slice.as_ptr()` is non-null and correctly
            //   aligned for `Array<T, S>` over a region of exactly the right size.
            // - Per the `ArrayType` invariant (reverse direction) those bytes are a valid
            //   `S::ArrayType<T>`, which `Array` is `repr(transparent)` over.
            // - The result borrows from `slice`, so it can neither dangle nor alias.
            unsafe { Ok(&*ptr) }
        } else {
            Err(TryFromSliceError(()))
        }
    }
}

impl<T, S: ArraySize> TryFrom<&mut [T]> for &mut Array<T, S> {
    type Error = TryFromSliceError;

    fn try_from(slice: &mut [T]) -> Result<Self, Self::Error> {
        const { <Array<T, S>>::LAYOUT_OK };
        if slice.len() == S::USIZE {
            let ptr: *mut Array<T, S> = slice.as_mut_ptr().cast();
            // SAFETY:
            // - `slice.len() == S::USIZE`, and `LAYOUT_OK` gives `Array<T, S>` the size and
            //   alignment of `[T; S::USIZE]`, so `slice.as_mut_ptr()` is non-null and
            //   correctly aligned for `Array<T, S>` over a region of exactly the right
            //   size.
            // - Per the `ArrayType` invariant (reverse direction) those bytes are a valid
            //   `S::ArrayType<T>`, which `Array` is `repr(transparent)` over.
            // - The result inherits uniqueness from the `&mut [T]` it borrows from, so it
            //   is the only live access to the storage for its lifetime.
            unsafe { Ok(&mut *ptr) }
        } else {
            Err(TryFromSliceError(()))
        }
    }
}

impl<T: Copy, S: ArraySize<ArrayType<T>: Copy>> TryFrom<&[T]> for Array<T, S> {
    type Error = TryFromSliceError;

    fn try_from(slice: &[T]) -> Result<Self, Self::Error> {
        <&Self>::try_from(slice).copied()
    }
}

mod sealed {
    use crate::{Concat, Sum, U};

    pub trait Sealed {}

    impl<T, const N: usize> Sealed for [T; N] {}
    impl<A, B> Sealed for Concat<A, B> {}
    impl<const N: usize> Sealed for U<N> {}
    impl<A, B> Sealed for Sum<A, B> {}
}
