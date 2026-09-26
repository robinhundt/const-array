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

use const_array::{Array, ArrayLen, Len, Prod, Sum, at_most, same_len};

// A moderately nested size used throughout: 2 + (3 + 1) = 6 elements.
type S6 = Sum<Len<2>, Sum<Len<3>, Len<1>>>;

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
    let mut arr: Array<i32, Sum<Len<2>, Len<2>>> = Array::from_fn(|i| i as i32);
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
    let mut arr: Array<u8, Len<4>> = [1, 2, 3, 4].into();
    arr[0] = 42;
    arr.iter_mut().for_each(|x| *x = x.wrapping_add(1));
    assert_eq!(&*arr, &[43, 3, 4, 5]);
}

#[test]
fn repeated_shared_then_mut_borrows_are_sound() {
    // Exercises Stacked/Tree Borrows: many shared reborrows followed by a
    // unique one, each freshly derived from the reference.
    let mut arr: Array<i32, Len<3>> = [1, 2, 3].into();
    let _ = arr.as_slice();
    let _ = arr.as_slice();
    arr.as_mut_slice()[0] = 9;
    assert_eq!(arr.as_slice(), &[9, 2, 3]);
}

#[test]
fn parts_splits_data_and_storage() {
    let arr: Array<i32, Sum<Len<3>, Len<4>>> = Array::from_fn(|i| i as i32);
    let (a, b) = arr.parts();
    assert_eq!(a.as_slice(), &[0, 1, 2]);
    assert_eq!(b.as_slice(), &[3, 4, 5, 6]);
}

#[test]
fn nested_parts() {
    let arr: Array<i32, Sum<Len<2>, Sum<Len<2>, Len<2>>>> = Array::from_fn(|i| i as i32);
    let (a, rest) = arr.parts();
    let (b, c) = rest.parts();
    assert_eq!(a.as_slice(), &[0, 1]);
    assert_eq!(b.as_slice(), &[2, 3]);
    assert_eq!(c.as_slice(), &[4, 5]);
}

#[test]
fn try_from_shared_slice_success_and_failure() {
    let data = [1i32, 2, 3, 4];

    let ok: Result<&Array<i32, Len<4>>, _> = (&data[..]).try_into();
    assert_eq!(ok.unwrap().as_slice(), &[1, 2, 3, 4]);

    let wrong: Result<&Array<i32, Len<3>>, _> = (&data[..]).try_into();
    assert!(wrong.is_err());
}

#[test]
fn try_from_mut_slice_writes_through() {
    let mut data = [1i32, 2, 3, 4];
    {
        let arr: &mut Array<i32, Len<4>> = (&mut data[..]).try_into().unwrap();
        arr[0] = 99;
        arr.as_mut_slice()[3] = 77;
    }
    // The write went back to the original buffer.
    assert_eq!(data, [99, 2, 3, 77]);

    let wrong: Result<&mut Array<i32, Len<3>>, _> = (&mut data[..]).try_into();
    assert!(wrong.is_err());
}

// The owned `TryFrom<&[T]>` clones the elements, so it only requires
// `T: Clone`, and works for `Sum` sizes too.
#[test]
fn try_from_owned_copy_roundtrips() {
    let data = [10u8, 20, 30, 40, 50];
    let arr: Array<u8, Sum<Len<2>, Len<3>>> = data[..].try_into().unwrap();
    assert_eq!(arr.as_slice(), &data);
}

// `Clone` on `Array` only requires `T: Clone`, so `Sum` sizes are cloneable
// too.
#[test]
#[allow(clippy::clone_on_copy)]
fn clone_is_a_deep_independent_copy() {
    let a: Array<i32, Sum<Len<2>, Len<2>>> = Array::from_fn(|i| i as i32);
    let mut b = a.clone();
    b.as_mut_slice()[0] = 100;
    // Mutating the clone must not touch the original's storage.
    assert_eq!(a.as_slice(), &[0, 1, 2, 3]);
    assert_eq!(b.as_slice(), &[100, 1, 2, 3]);
}

// A `Sum`-sized array of `Copy` elements is `Copy`, as `Concat` is: passing it
// by value leaves the source usable, and the copy is an independent duplicate.
#[test]
fn sum_sized_array_is_copy() {
    let a: Array<i32, Sum<Len<2>, Len<2>>> = Array::from_fn(|i| i as i32);
    let mut b = a;
    let c = a; // still usable => `a` was copied, not moved
    b.as_mut_slice()[0] = 100;
    assert_eq!(a.as_slice(), &[0, 1, 2, 3]);
    assert_eq!(b.as_slice(), &[100, 1, 2, 3]);
    assert_eq!(c.as_slice(), &[0, 1, 2, 3]);
}

#[test]
fn empty_array_is_sound() {
    let arr: Array<i32, Len<0>> = Array::from_fn(|i| i as i32);
    assert!(arr.as_slice().is_empty());
    assert!(arr.is_empty());
    assert_eq!(size_of_val(&arr), 0);
}

#[test]
fn zero_sized_elements() {
    let arr: Array<(), Sum<Len<3>, Len<2>>> = Array::from_fn(|_| ());
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
        let arr: Array<Bomb, Sum<Len<2>, Len<2>>> = Array::from_fn(|_| Bomb {
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

fn bombs<S: ArrayLen>(counter: &Rc<Cell<usize>>) -> Array<Bomb, S> {
    Array::from_fn(|_| Bomb {
        counter: Rc::clone(counter),
    })
}

#[test]
fn cast_moves_ownership_without_extra_drops() {
    let counter = Rc::new(Cell::new(0));
    {
        let arr: Array<Bomb, S6> = bombs(&counter);
        let cast: Array<Bomb, Len<6>> = arr.cast(same_len!(S6, Len<6>));
        assert_eq!(counter.get(), 0, "casting must not drop elements");
        drop(cast);
        assert_eq!(counter.get(), 6);
    }
    assert_eq!(counter.get(), 6);
}

#[test]
fn cast_preserves_elements_and_storage() {
    let mut arr: Array<i32, S6> = Array::from_fn(|i| i as i32);
    let ptr = arr.as_slice().as_ptr();

    let view: &Array<i32, Sum<Len<3>, Len<3>>> = arr.cast_ref(same_len!(S6, Sum<Len<3>, Len<3>>));
    assert_eq!(view.as_slice().as_ptr(), ptr);
    let (a, b) = view.split_ref();
    assert_eq!(
        (a.as_slice(), b.as_slice()),
        (&[0, 1, 2][..], &[3, 4, 5][..])
    );

    let view_mut: &mut Array<i32, Len<6>> = arr.cast_mut(same_len!(S6, Len<6>));
    view_mut[5] = 50;
    assert_eq!(arr[5], 50);

    let owned: Array<i32, Len<6>> = arr.cast(same_len!(S6, Len<6>));
    assert_eq!(owned.as_slice(), &[0, 1, 2, 3, 4, 50]);
}

#[test]
fn try_cast_failure_returns_array_without_drops() {
    let counter = Rc::new(Cell::new(0));
    {
        let arr: Array<Bomb, S6> = bombs(&counter);
        let Err(arr) = arr.try_cast::<Len<5>>() else {
            panic!("lengths differ");
        };
        assert_eq!(counter.get(), 0, "a failed cast must not drop elements");
        let arr = arr.try_cast::<Len<6>>().ok().unwrap();
        assert_eq!(arr.len(), 6);
        assert_eq!(counter.get(), 0);
    }
    assert_eq!(counter.get(), 6);
}

#[test]
fn split_mut_writes_into_the_original() {
    let mut arr: Array<i32, S6> = Array::from_fn(|i| i as i32);
    let ptr = arr.as_slice().as_ptr();
    let (a, rest) = arr.split_mut();
    let (b, c) = rest.split_mut();
    a[0] = 10;
    b[0] = 20;
    c[0] = 30;
    assert_eq!(arr.as_slice(), &[10, 1, 20, 3, 4, 30]);
    assert_eq!(arr.as_slice().as_ptr(), ptr);
}

#[test]
fn concat_moves_ownership_without_extra_drops() {
    let counter = Rc::new(Cell::new(0));
    {
        let a: Array<Bomb, Len<2>> = bombs(&counter);
        let b: Array<Bomb, Sum<Len<3>, Len<1>>> = bombs(&counter);
        let arr: Array<Bomb, S6> = Array::concat(a, b);
        assert_eq!(counter.get(), 0);
        assert_eq!(arr.len(), 6);
    }
    assert_eq!(counter.get(), 6);
}

#[test]
fn into_iter_partial_consumption_drops_the_rest_once() {
    let counter = Rc::new(Cell::new(0));
    {
        let arr: Array<Bomb, S6> = bombs(&counter);
        let mut iter = arr.into_iter();
        assert_eq!(iter.len(), 6);
        drop(iter.next());
        drop(iter.next_back());
        assert_eq!(counter.get(), 2);
        assert_eq!(iter.len(), 4);
        assert_eq!(iter.as_slice().len(), 4);
    }
    assert_eq!(counter.get(), 6);
}

#[test]
fn into_iter_yields_all_elements_in_order() {
    let arr: Array<String, S6> = Array::from_fn(|i| i.to_string());
    let mut iter = arr.into_iter();
    assert_eq!(iter.next_back().as_deref(), Some("5"));
    let rest: Vec<String> = iter.collect();
    assert_eq!(rest, ["0", "1", "2", "3", "4"]);
}

#[test]
fn into_iter_as_mut_slice_writes_through() {
    let arr: Array<String, S6> = Array::from_fn(|i| i.to_string());
    let mut iter = arr.into_iter();
    iter.next();
    iter.next_back();
    let alive = iter.as_mut_slice();
    assert_eq!(alive, ["1", "2", "3", "4"]);
    for s in alive {
        s.push('!');
    }
    // Reading through a shared view after the mutable one must be sound.
    assert_eq!(iter.as_slice(), ["1!", "2!", "3!", "4!"]);
    assert_eq!(iter.next().as_deref(), Some("1!"));
    assert_eq!(iter.next_back().as_deref(), Some("4!"));
    assert_eq!(iter.as_mut_slice(), ["2!", "3!"]);
}

#[test]
fn map_moves_every_element_once() {
    let counter = Rc::new(Cell::new(0));
    {
        let arr: Array<Bomb, S6> = bombs(&counter);
        let mapped: Array<Rc<Cell<usize>>, S6> = arr.map(|b| Rc::clone(&b.counter));
        // Each `Bomb` is dropped at the end of the closure.
        assert_eq!(counter.get(), 6);
        assert_eq!(mapped.len(), 6);
    }
    assert_eq!(counter.get(), 6);
}

#[test]
fn map_panic_drops_everything_once() {
    let counter = Rc::new(Cell::new(0));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let arr: Array<Bomb, S6> = bombs(&counter);
        let mut calls = 0;
        // Panic while mapping the 4th element, which is in the second half of
        // the outer `Sum`: 3 elements are mapped, 1 is in the closure,
        // 2 are unvisited.
        arr.map(|b| {
            calls += 1;
            assert!(calls < 4, "boom");
            b
        })
    }));
    assert!(result.is_err());
    assert_eq!(counter.get(), 6, "no leaks and no double drops on panic");
}

#[test]
fn try_from_fn_stops_at_the_first_error() {
    let counter = Rc::new(Cell::new(0));
    let mut calls = 0;
    let result: Result<Array<Bomb, S6>, usize> = Array::try_from_fn(|i| {
        calls += 1;
        if i == 3 {
            Err(i)
        } else {
            Ok(Bomb {
                counter: Rc::clone(&counter),
            })
        }
    });
    assert_eq!(result.err(), Some(3));
    assert_eq!(calls, 4, "no calls after the error");
    assert_eq!(
        counter.get(),
        3,
        "the elements built so far are dropped once"
    );
}

#[test]
fn into_array_moves_ownership_without_extra_drops() {
    let counter = Rc::new(Cell::new(0));
    {
        let arr: Array<Bomb, Len<3>> = bombs(&counter);
        let raw: [Bomb; 3] = arr.into_array();
        assert_eq!(counter.get(), 0);
        let arr = Array::new(raw);
        assert_eq!(counter.get(), 0);
        drop(arr);
        assert_eq!(counter.get(), 3);
    }
    assert_eq!(counter.get(), 3);
}

#[test]
fn from_mut_writes_through() {
    let mut raw = [1i32, 2, 3, 4];
    let arr = Array::from_mut(&mut raw);
    let (a, b) = arr
        .cast_mut(same_len!(Len<4>, Sum<Len<1>, Len<3>>))
        .split_mut();
    a[0] = 10;
    b[2] = 40;
    assert_eq!(Array::from_ref(&raw).as_array(), &[10, 2, 3, 40]);
}

#[test]
fn cast_checked_moves_ownership_without_extra_drops() {
    let counter = Rc::new(Cell::new(0));
    {
        let arr: Array<Bomb, S6> = bombs(&counter);
        let cast: Array<Bomb, Len<6>> = arr.cast_checked();
        assert_eq!(counter.get(), 0);
        drop(cast);
        assert_eq!(counter.get(), 6);
    }
    assert_eq!(counter.get(), 6);
}

// A nested product: 3 chunks of (1 + 1) elements.
type P6 = Prod<Len<3>, Sum<Len<1>, Len<1>>>;

#[test]
fn prod_from_fn_is_row_major() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let arr: Array<usize, P6> = Array::from_fn(|i| {
        seen.borrow_mut().push(i);
        i
    });
    assert_eq!(*seen.borrow(), (0..6).collect::<Vec<_>>());
    assert_eq!(arr.as_slice(), &[0, 1, 2, 3, 4, 5]);
    let chunks = arr.as_chunks();
    assert_eq!(chunks.len(), 3);
    for (j, chunk) in chunks.iter().enumerate() {
        assert_eq!(chunk.as_slice(), &[2 * j, 2 * j + 1]);
    }
}

#[test]
fn prod_as_chunks_mut_writes_into_the_original() {
    let mut arr: Array<i32, P6> = Array::from_fn(|i| i as i32);
    let ptr = arr.as_slice().as_ptr();
    let chunks = arr.as_chunks_mut();
    assert_eq!(chunks.as_slice().as_ptr().cast(), ptr);
    chunks[1][1] = 30;
    let (a, _) = chunks[2].split_mut();
    a[0] = 40;
    assert_eq!(arr.as_slice(), &[0, 1, 2, 30, 40, 5]);
}

#[test]
fn prod_chunks_roundtrip_without_extra_drops() {
    let counter = Rc::new(Cell::new(0));
    {
        let arr: Array<Bomb, P6> = bombs(&counter);
        let mut chunks = arr.into_chunks().into_iter();
        assert_eq!(counter.get(), 0, "splitting must not drop elements");
        drop(chunks.next());
        assert_eq!(counter.get(), 2);
        let rest: Array<_, Len<2>> = Array::from_fn(|_| chunks.next().unwrap());
        let arr: Array<Bomb, Prod<Len<2>, Sum<Len<1>, Len<1>>>> = Array::from_chunks(rest);
        assert_eq!(counter.get(), 2, "flattening must not drop elements");
        assert_eq!(arr.len(), 4);
    }
    assert_eq!(counter.get(), 6);
}

#[test]
fn prod_from_fn_panic_drops_built_elements_once() {
    let counter = Rc::new(Cell::new(0));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Array<Bomb, P6> = Array::from_fn(|i| {
            if i == 3 {
                panic!("boom");
            }
            Bomb {
                counter: Rc::clone(&counter),
            }
        });
    }));
    assert!(result.is_err());
    assert_eq!(counter.get(), 3, "the 3 built elements are dropped once");
}

#[test]
fn prod_clone_of_non_copy_elements() {
    let arr: Array<String, P6> = Array::from_fn(|i| i.to_string());
    let cloned = arr.clone();
    assert_eq!(arr.as_slice(), cloned.as_slice());
    assert_ne!(arr[4].as_ptr(), cloned[4].as_ptr());
}

#[test]
fn prod_of_zsts() {
    let arr: Array<(), Prod<Len<3>, Len<2>>> = Array::from_fn(|_| ());
    assert_eq!(arr.len(), 6);
    assert_eq!(arr.as_chunks().len(), 3);
    assert_eq!(arr.into_chunks()[2].len(), 2);
}

#[test]
fn truncate_drops_the_rest_exactly_once() {
    let counter = Rc::new(Cell::new(0));
    {
        let arr: Array<Bomb, S6> = bombs(&counter);
        let short: Array<Bomb, Len<2>> = arr.truncate(at_most!(Len<2>, S6));
        assert_eq!(counter.get(), 4, "the 4 truncated elements are dropped");
        drop(short);
        assert_eq!(counter.get(), 6);
    }
    assert_eq!(counter.get(), 6);
}

// Panics when dropped, if `armed`.
struct PanicOnDrop {
    armed: bool,
    counter: Rc<Cell<usize>>,
}

impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        self.counter.set(self.counter.get() + 1);
        assert!(!self.armed, "boom");
    }
}

#[test]
fn truncate_panicking_drop_still_drops_everything_once() {
    let counter = Rc::new(Cell::new(0));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // The 4th element, the second of the truncated ones, panics on drop.
        let arr: Array<PanicOnDrop, S6> = Array::from_fn(|i| PanicOnDrop {
            armed: i == 3,
            counter: Rc::clone(&counter),
        });
        arr.truncate(at_most!(Len<2>, S6))
    }));
    assert!(result.is_err());
    assert_eq!(counter.get(), 6, "no leaks and no double drops on panic");
}

#[test]
fn prefix_views_point_into_the_array() {
    let mut arr: Array<i32, S6> = Array::from_fn(|i| i as i32);
    let ptr = arr.as_slice().as_ptr();

    let prefix = arr.prefix_ref(at_most!(Len<4>, S6));
    assert_eq!(
        (prefix.as_slice().as_ptr(), prefix.as_array()),
        (ptr, &[0, 1, 2, 3])
    );

    let (head, rest) = arr.split_prefix(at_most!(Len<2>, S6));
    assert_eq!((head.as_array(), rest), (&[0, 1], &[2, 3, 4, 5][..]));

    arr.prefix_mut(at_most!(Len<1>, S6))[0] = 10;
    let (head, rest) = arr.split_prefix_mut(at_most!(Len<3>, S6));
    head[2] = 20;
    rest[2] = 30;
    assert_eq!(arr.as_slice(), &[10, 1, 20, 3, 4, 30]);
    assert_eq!(arr.as_slice().as_ptr(), ptr);

    // The whole array and nothing are valid prefixes.
    let (all, none) = arr.split_prefix(at_most!(S6, S6));
    assert_eq!((all.len(), none.len()), (6, 0));
    let (empty, whole) = arr.split_prefix(at_most!(Len<0>, S6));
    assert_eq!((empty.len(), whole.len()), (0, 6));
}

#[test]
fn pad_from_moves_the_prefix_and_clones_the_fill() {
    let arr: Array<String, S6> = Array::pad_from(
        Array::from(["a".to_string(), "b".to_string()]),
        at_most!(Len<2>, S6),
        "-".to_string(),
    );
    assert_eq!(arr.as_slice(), ["a", "b", "-", "-", "-", "-"]);

    let counter = Rc::new(Cell::new(0));
    {
        let prefix: Array<Bomb, Len<2>> = bombs(&counter);
        let fill = Bomb {
            counter: Rc::clone(&counter),
        };
        let _padded: Array<Rc<Cell<usize>>, Len<3>> = Array::pad_from(
            prefix.map(|b| Rc::clone(&b.counter)),
            at_most!(Len<2>, Len<3>),
            Rc::clone(&fill.counter),
        );
        assert_eq!(counter.get(), 2, "only the mapped bombs are dropped");
    }
    assert_eq!(counter.get(), 3);
}

#[test]
fn slice_as_chunks_points_into_the_slice() {
    let data: Vec<i32> = (0..14).collect();
    let (chunks, rest) = Array::<i32, S6>::slice_as_chunks(&data);
    assert_eq!((chunks.len(), rest), (2, &[12, 13][..]));
    assert_eq!(chunks[0].as_slice().as_ptr(), data.as_ptr());
    assert_eq!(chunks[1].as_slice(), &data[6..12]);

    let (chunks, rest) = Array::<i32, S6>::slice_as_chunks(&data[..5]);
    assert_eq!((chunks.len(), rest.len()), (0, 5));
    let (chunks, rest) = Array::<i32, S6>::slice_as_chunks(&[]);
    assert_eq!((chunks.len(), rest.len()), (0, 0));

    let zsts = [(); 7];
    let (chunks, rest) = Array::<(), Len<3>>::slice_as_chunks(&zsts);
    assert_eq!((chunks.len(), rest.len()), (2, 1));
}

#[test]
fn slice_as_chunks_mut_writes_into_the_slice() {
    let mut data: Vec<i32> = (0..14).collect();
    let (chunks, rest) = Array::<i32, S6>::slice_as_chunks_mut(&mut data);
    let (a, _) = chunks[1].split_mut();
    a[1] = 70;
    chunks[0][5] = 50;
    rest[1] = 130;
    assert_eq!(data[5], 50);
    assert_eq!(data[7], 70);
    assert_eq!(data[13], 130);

    // A length whose quotient and remainder differ.
    let (chunks, rest) = Array::<i32, S6>::slice_as_chunks_mut(&mut data[..13]);
    assert_eq!((chunks.len(), rest.len()), (2, 1));
}

#[test]
fn suffix_views_point_into_the_array() {
    let mut arr: Array<i32, S6> = Array::from_fn(|i| i as i32);
    let ptr = arr.as_slice().as_ptr();

    let suffix = arr.suffix_ref(at_most!(Len<4>, S6));
    assert_eq!(
        (suffix.as_slice().as_ptr(), suffix.as_array()),
        (ptr.wrapping_add(2), &[2, 3, 4, 5])
    );

    let (rest, tail) = arr.split_suffix(at_most!(Len<2>, S6));
    assert_eq!((rest, tail.as_array()), (&[0, 1, 2, 3][..], &[4, 5]));

    arr.suffix_mut(at_most!(Len<1>, S6))[0] = 50;
    let (rest, tail) = arr.split_suffix_mut(at_most!(Len<3>, S6));
    tail[0] = 30;
    rest[0] = 10;
    assert_eq!(arr.as_slice(), &[10, 1, 2, 30, 4, 50]);
    assert_eq!(arr.as_slice().as_ptr(), ptr);

    // The whole array and nothing are valid suffixes.
    let (none, all) = arr.split_suffix(at_most!(S6, S6));
    assert_eq!((none.len(), all.len()), (0, 6));
    let (whole, empty) = arr.split_suffix(at_most!(Len<0>, S6));
    assert_eq!((whole.len(), empty.len()), (6, 0));
    assert_eq!(empty.as_slice().as_ptr(), ptr.wrapping_add(6));
}
