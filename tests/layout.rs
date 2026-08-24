//! Layout / post-monomorphization invariant tests.
//!
//! The `unsafe` code in `as_slice`/`as_mut_slice`/`TryFrom` reinterprets an
//! `Array<T, S>` as `[T; S::USIZE]`. That is only sound if, *after
//! monomorphization*, three invariants hold for the concrete types:
//!
//!   1. `size_of::<Array<T, S>>() == S::USIZE * size_of::<T>()`
//!   2. `align_of::<Array<T, S>>() == align_of::<T>()`
//!   3. `<S::ArrayType<T> as ArrayType<T>>::LEN == S::USIZE`
//!
//! If any nested `Concat`/`Sum` combination introduced padding or a length
//! mismatch, the raw-slice construction would be UB. These tests assert the
//! invariants across a matrix of element types and (possibly deeply nested)
//! sizes. A violation would surface here (and, for #1/#2, as UB under miri in
//! the soundness tests).

use core::mem::{align_of, size_of};

use const_array::{Array, ArraySize, ArrayType, Concat, Sum, U};

fn check_invariants<T, S: ArraySize>() {
    assert_eq!(
        size_of::<Array<T, S>>(),
        S::USIZE * size_of::<T>(),
        "size_of::<Array<T, S>> must equal USIZE * size_of::<T>",
    );
    assert_eq!(
        align_of::<Array<T, S>>(),
        align_of::<T>(),
        "an Array must have the same alignment as its element",
    );
    assert_eq!(
        <S::ArrayType<T> as ArrayType<T>>::LEN,
        S::USIZE,
        "the concrete ArrayType length must match the declared USIZE",
    );
}

macro_rules! check_over_sizes {
    ($($t:ty),* $(,)?) => {{
        $(
            check_invariants::<$t, U<0>>();
            check_invariants::<$t, U<1>>();
            check_invariants::<$t, U<5>>();
            check_invariants::<$t, Sum<U<1>, U<1>>>();
            check_invariants::<$t, Sum<U<3>, U<4>>>();
            check_invariants::<$t, Sum<U<0>, U<7>>>();
            check_invariants::<$t, Sum<U<1>, Sum<U<2>, U<3>>>>();
            check_invariants::<$t, Sum<Sum<U<2>, U<2>>, Sum<U<1>, U<3>>>>();
            check_invariants::<$t, Sum<Sum<Sum<U<1>, U<1>>, U<1>>, U<1>>>();
        )*
    }};
}

// A padded element: `repr(C)` gives size 8, align 4 (a byte of trailing pad).
#[derive(Clone, Copy)]
#[repr(C)]
struct Padded {
    _a: u8,
    _b: u32,
}

// An over-aligned element: size is rounded up to the alignment.
#[derive(Clone, Copy)]
#[repr(align(16))]
struct Aligned(#[allow(dead_code)] u8);

#[test]
fn layout_invariants_across_types_and_sizes() {
    check_over_sizes!(u8, u16, u32, u64, u128, i8, isize, (), Padded, Aligned);
}

#[test]
fn concat_is_contiguous_and_padding_free() {
    // Directly pin down that `Concat` matches the flat array it stands in for.
    assert_eq!(
        size_of::<Concat<[u32; 3], [u32; 5]>>(),
        size_of::<[u32; 8]>()
    );
    assert_eq!(align_of::<Concat<[u32; 3], [u32; 5]>>(), align_of::<u32>());

    // Nested on the right and left.
    assert_eq!(
        size_of::<Concat<[u8; 1], Concat<[u8; 2], [u8; 4]>>>(),
        size_of::<[u8; 7]>(),
    );
    assert_eq!(
        size_of::<Concat<[Padded; 1], Concat<[Padded; 1], [Padded; 1]>>>(),
        3 * size_of::<Padded>(),
    );
}

#[test]
fn usize_matches_manual_sum() {
    assert_eq!(<U<0> as ArraySize>::USIZE, 0);
    assert_eq!(<U<5> as ArraySize>::USIZE, 5);
    assert_eq!(<Sum<U<3>, U<4>> as ArraySize>::USIZE, 7);
    assert_eq!(<Sum<U<1>, Sum<U<2>, U<3>>> as ArraySize>::USIZE, 6);
}
