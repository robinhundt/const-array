//! Behavioral / soundness tests (the miri-focused suite).
//!
//! These drive the `unsafe` pointer reinterpretation code down real execution
//! paths: constructing arrays, viewing them as slices, mutating through those
//! slices, splitting, converting from slices, and dropping. Run under miri to
//! catch out-of-bounds reads, misaligned accesses, aliasing (Stacked/Tree
//! Borrows) violations, uninitialized reads, and leaks / double drops:
//!
//!     cargo +nightly miri test

use core::mem::{align_of, align_of_val, size_of_val};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use const_array::{Array, Sum, U};

// A moderately nested size used throughout: 2 + (3 + 1) = 6 elements.
type S6 = Sum<U<2>, Sum<U<3>, U<1>>>;

#[test]
fn from_fn_roundtrips_through_slice() {
    let arr: Array<i32, S6> = Array::from_fn(|i| i as i32);
    assert_eq!(arr.as_slice(), &[0, 1, 2, 3, 4, 5]);
}

#[test]
fn from_fn_visits_indices_in_order() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let arr: Array<usize, S6> = Array::from_fn(|i| {
        seen.borrow_mut().push(i);
        i
    });
    // The closure must see 0..USIZE exactly once, in order, across the nesting.
    assert_eq!(*seen.borrow(), (0..6).collect::<Vec<_>>());
    assert_eq!(arr.as_slice(), &[0, 1, 2, 3, 4, 5]);
}

#[test]
fn as_slice_points_into_the_array() {
    let arr: Array<i32, S6> = Array::from_fn(|i| i as i32);
    let base: *const i32 = (&arr as *const Array<i32, S6>).cast();
    // No copy: the slice aliases the array's own storage.
    assert_eq!(arr.as_slice().as_ptr(), base);
}

#[test]
fn as_mut_slice_mutates_in_place() {
    let mut arr: Array<i32, Sum<U<2>, U<2>>> = Array::from_fn(|i| i as i32);
    let ptr_before = arr.as_slice().as_ptr();
    for x in arr.as_mut_slice() {
        *x *= 10;
    }
    assert_eq!(arr.as_slice(), &[0, 10, 20, 30]);
    // Mutation happened through the same storage, no reallocation.
    assert_eq!(arr.as_slice().as_ptr(), ptr_before);
}

#[test]
fn deref_mut_writes_through() {
    let mut arr: Array<u8, U<4>> = [1, 2, 3, 4].into();
    arr[0] = 42;
    arr.iter_mut().for_each(|x| *x = x.wrapping_add(1));
    assert_eq!(&*arr, &[43, 3, 4, 5]);
}

#[test]
fn repeated_shared_then_mut_borrows_are_sound() {
    // Exercises Stacked/Tree Borrows: many shared reborrows followed by a
    // unique one, each freshly derived from the reference.
    let mut arr: Array<i32, U<3>> = [1, 2, 3].into();
    let _ = arr.as_slice();
    let _ = arr.as_slice();
    arr.as_mut_slice()[0] = 9;
    assert_eq!(arr.as_slice(), &[9, 2, 3]);
}

#[test]
fn parts_splits_data_and_storage() {
    let arr: Array<i32, Sum<U<3>, U<4>>> = Array::from_fn(|i| i as i32);
    let (a, b) = arr.parts();
    assert_eq!(a.as_slice(), &[0, 1, 2]);
    assert_eq!(b.as_slice(), &[3, 4, 5, 6]);
}

#[test]
fn nested_parts() {
    let arr: Array<i32, Sum<U<2>, Sum<U<2>, U<2>>>> = Array::from_fn(|i| i as i32);
    let (a, rest) = arr.parts();
    let (b, c) = rest.parts();
    assert_eq!(a.as_slice(), &[0, 1]);
    assert_eq!(b.as_slice(), &[2, 3]);
    assert_eq!(c.as_slice(), &[4, 5]);
}

#[test]
fn try_from_shared_slice_success_and_failure() {
    let data = [1i32, 2, 3, 4];

    let ok: Result<&Array<i32, U<4>>, _> = (&data[..]).try_into();
    assert_eq!(ok.unwrap().as_slice(), &[1, 2, 3, 4]);

    let wrong: Result<&Array<i32, U<3>>, _> = (&data[..]).try_into();
    assert!(wrong.is_err());
}

#[test]
fn try_from_mut_slice_writes_through() {
    let mut data = [1i32, 2, 3, 4];
    {
        let arr: &mut Array<i32, U<4>> = (&mut data[..]).try_into().unwrap();
        arr[0] = 99;
        arr.as_mut_slice()[3] = 77;
    }
    // The write went back to the original buffer.
    assert_eq!(data, [99, 2, 3, 77]);
}

// The owned `TryFrom<&[T]>` requires `S::ArrayType<T>: Copy`; since `Concat`
// is `Copy`, it works for `Sum` sizes too.
#[test]
fn try_from_owned_copy_roundtrips() {
    let data = [10u8, 20, 30, 40, 50];
    let arr: Array<u8, Sum<U<2>, U<3>>> = data[..].try_into().unwrap();
    assert_eq!(arr.as_slice(), &data);
}

// `Clone`/`Copy` on `Array` require `S::ArrayType<T>: Clone`/`Copy`; since
// `Concat` is `Clone`, `Sum` sizes are cloneable too.
#[test]
fn clone_is_a_deep_independent_copy() {
    let a: Array<i32, Sum<U<2>, U<2>>> = Array::from_fn(|i| i as i32);
    let mut b = a.clone();
    b.as_mut_slice()[0] = 100;
    // Mutating the clone must not touch the original's storage.
    assert_eq!(a.as_slice(), &[0, 1, 2, 3]);
    assert_eq!(b.as_slice(), &[100, 1, 2, 3]);
}

// A `Sum`-sized array is now `Copy` (via `Concat: Copy`): passing it by value
// leaves the source usable, and the copy is an independent duplicate.
#[test]
fn sum_sized_array_is_copy() {
    let a: Array<i32, Sum<U<2>, U<2>>> = Array::from_fn(|i| i as i32);
    let mut b = a;
    let c = a; // still usable => `a` was copied, not moved
    b.as_mut_slice()[0] = 100;
    assert_eq!(a.as_slice(), &[0, 1, 2, 3]);
    assert_eq!(b.as_slice(), &[100, 1, 2, 3]);
    assert_eq!(c.as_slice(), &[0, 1, 2, 3]);
}

#[test]
fn empty_array_is_sound() {
    let arr: Array<i32, U<0>> = Array::from_fn(|i| i as i32);
    assert!(arr.as_slice().is_empty());
    assert!(arr.is_empty());
    assert_eq!(size_of_val(&arr), 0);
}

#[test]
fn zero_sized_elements() {
    let arr: Array<(), Sum<U<3>, U<2>>> = Array::from_fn(|_| ());
    assert_eq!(arr.as_slice().len(), 5);
    assert_eq!(size_of_val(&arr), 0);
    // Alignment is still that of the element.
    assert_eq!(align_of_val(&arr), align_of::<()>());
}

// Increments a shared counter on drop, so we can prove every element is
// dropped exactly once (miri also flags leaks and double drops directly).
struct Bomb {
    counter: Rc<Cell<usize>>,
}

impl Drop for Bomb {
    fn drop(&mut self) {
        self.counter.set(self.counter.get() + 1);
    }
}

#[test]
fn every_element_is_dropped_exactly_once() {
    let counter = Rc::new(Cell::new(0));
    {
        let _arr: Array<Bomb, S6> = Array::from_fn(|_| Bomb {
            counter: Rc::clone(&counter),
        });
        assert_eq!(counter.get(), 0, "nothing dropped while alive");
    }
    assert_eq!(counter.get(), 6, "each of the 6 elements dropped once");
}

#[test]
fn parts_moves_ownership_without_extra_drops() {
    let counter = Rc::new(Cell::new(0));
    {
        let arr: Array<Bomb, Sum<U<2>, U<2>>> = Array::from_fn(|_| Bomb {
            counter: Rc::clone(&counter),
        });
        let (a, b) = arr.parts();
        assert_eq!(counter.get(), 0, "splitting must not drop elements");
        drop(a);
        assert_eq!(counter.get(), 2);
        drop(b);
        assert_eq!(counter.get(), 4);
    }
    assert_eq!(counter.get(), 4);
}
