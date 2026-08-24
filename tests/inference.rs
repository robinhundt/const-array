//! Type-inference focused tests.
//!
//! These exercise the ergonomics of the public API: whether the compiler can
//! infer element types, sizes, and sub-array types from the surrounding
//! context (binding annotations, turbofish, `From`/`TryFrom`, `Deref`, and
//! `parts`). They are deliberately light on runtime assertions -- the point is
//! that the code type-checks -- but they still run cleanly under miri.

use const_array::{Array, Sum, U};

// Readable size aliases.
type S3 = U<3>;
type S6 = Sum<U<3>, U<3>>;
type S7 = Sum<U<1>, Sum<U<2>, U<4>>>;

#[test]
fn from_fn_infers_from_binding_annotation() {
    // Element type (`i32`) and size (`S3`) both come from the annotation.
    let a: Array<i32, S3> = Array::from_fn(|i| i as i32);
    assert_eq!(a.as_slice(), &[0, 1, 2]);
}

#[test]
fn from_fn_infers_from_turbofish() {
    let a = Array::<u8, S3>::from_fn(|i| i as u8);
    assert_eq!(&*a, &[0u8, 1, 2]);
}

#[test]
fn into_and_from_array() {
    // `From<[T; N]> for Array<T, U<N>>` drives inference in both directions.
    let a: Array<i32, U<3>> = [10, 20, 30].into();
    assert_eq!(&*a, &[10, 20, 30]);

    let b = Array::from([1, 2, 3]);
    let _: Array<i32, U<3>> = b;
}

#[test]
fn deref_to_slice_methods() {
    // `Deref<Target = [T]>` makes slice inherent methods available directly.
    let a: Array<i32, S6> = Array::from_fn(|i| i as i32);
    assert_eq!(a.len(), 6);
    assert_eq!(a[5], 5);
    let sum: i32 = a.iter().sum();
    assert_eq!(sum, 15);
}

#[test]
fn try_from_slice_ref_infers() {
    let data = [1, 2, 3, 4, 5, 6];
    let slice: &[i32] = &data;

    let arr: &Array<i32, S6> = slice.try_into().unwrap();
    assert_eq!(arr.len(), 6);

    // A mismatched size infers a different target and simply errors.
    let bad: Result<&Array<i32, S3>, _> = slice.try_into();
    assert!(bad.is_err());
}

#[test]
fn try_from_slice_owned_copy() {
    // The owned `TryFrom` exists for `Copy` element types and any size whose
    // `ArrayType` is `Copy` -- including `Sum` sizes (`Concat: Copy`).
    let data = [1u8, 2, 3, 4];
    let arr: Array<u8, Sum<U<1>, U<3>>> = data[..].try_into().unwrap();
    assert_eq!(&*arr, &[1, 2, 3, 4]);
}

#[test]
fn parts_infers_subtypes() {
    let a: Array<i32, Sum<U<2>, U<3>>> = Array::from_fn(|i| i as i32);
    let (head, tail) = a.parts();
    // The split-out halves infer their sizes from the `Sum`.
    let _: Array<i32, U<2>> = head;
    let _: Array<i32, U<3>> = tail;
    assert_eq!(&*head, &[0, 1]);
    assert_eq!(&*tail, &[2, 3, 4]);
}

#[test]
fn default_infers() {
    let a: Array<i32, S7> = Default::default();
    assert_eq!(a.len(), 7);
    assert!(a.iter().all(|&x| x == 0));
}

#[test]
fn copy_leaves_source_usable() {
    // Requires `Array<i32, Sum<U<1>, U<2>>>: Copy`, which needs the concrete
    // `Concat<[i32; 1], [i32; 2]>: Copy`.
    let a: Array<i32, Sum<U<1>, U<2>>> = Array::from_fn(|i| i as i32);
    let b = a;
    let c = a; // still usable => `a` was copied, not moved
    assert_eq!(b.as_slice(), c.as_slice());
}
