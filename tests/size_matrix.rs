//! The same behavioral tests, run over many sizes and element types.
//!
//! Every size in the matrix below has a different structure (plain, nested
//! `Sum`s and `Prod`s, empty parts), and each is paired with a flat `Len<N>`
//! of the same length. The tests are generic, so they exercise the unsafe code
//! with every structure. Element types cover zero-sized, over-aligned, heap
//! owning and drop-tracking types.
//!
//! The panic tests inject a panic at *every* index of every operation that
//! builds elements, and check that no element is leaked or dropped twice.
//! Run under miri to also catch undefined behavior on these paths.

use std::{
    cell::Cell,
    fmt::Debug,
    hash::{DefaultHasher, Hash, Hasher},
    mem::align_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

use const_array::{Array, ArrayLen, AtMost, Len, Prod, SameLen, Sum};

/// An element type that can be built from its index.
trait Elem: Clone + PartialEq + Debug + Hash {
    fn make(i: usize) -> Self;
}

impl Elem for () {
    fn make(_: usize) -> Self {}
}

impl Elem for u8 {
    fn make(i: usize) -> Self {
        i as u8
    }
}

impl Elem for String {
    fn make(i: usize) -> Self {
        i.to_string()
    }
}

/// Over-aligned, so its size is 64 bytes and misplaced elements are likely to
/// be misaligned.
#[derive(Clone, PartialEq, Debug, Hash)]
#[repr(align(64))]
struct Aligned(u8);

impl Elem for Aligned {
    fn make(i: usize) -> Self {
        Aligned(i as u8)
    }
}

thread_local! {
    /// The number of `Tracked` values alive on this thread.
    static LIVE: Cell<usize> = const { Cell::new(0) };
    /// The number of `Tracked` values that can still be created before the
    /// next creation panics, or `None` to never panic.
    static FUSE: Cell<Option<usize>> = const { Cell::new(None) };
}

fn live() -> usize {
    LIVE.get()
}

/// Counts how many values are alive, and panics on creation when the fuse
/// runs out.
#[derive(PartialEq, Debug, Hash)]
struct Tracked(usize);

impl Tracked {
    fn new(i: usize) -> Self {
        match FUSE.get() {
            Some(0) => {
                FUSE.set(None);
                // Unlike `panic!`, this doesn't print a message for every
                // injected panic.
                resume_unwind(Box::new("injected panic"));
            }
            Some(n) => FUSE.set(Some(n - 1)),
            None => {}
        }
        LIVE.set(LIVE.get() + 1);
        Tracked(i)
    }
}

impl Clone for Tracked {
    fn clone(&self) -> Self {
        Tracked::new(self.0)
    }
}

impl Drop for Tracked {
    fn drop(&mut self) {
        let live = LIVE.get();
        assert!(live > 0, "a value was dropped twice");
        LIVE.set(live - 1);
    }
}

impl Elem for Tracked {
    fn make(i: usize) -> Self {
        Tracked::new(i)
    }
}

/// Run `f`, allowing only `k` `Tracked` values to be created. Returns `None`
/// if it panicked.
fn with_fuse<R>(k: usize, f: impl FnOnce() -> R) -> Option<R> {
    FUSE.set(Some(k));
    let result = catch_unwind(AssertUnwindSafe(f)).ok();
    FUSE.set(None);
    result
}

fn hash<H: Hash + ?Sized>(value: &H) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

/// Checks the values that the operations produce. `F` is a flat size of the
/// same length as `S`.
fn check_values<T: Elem, S: ArrayLen, F: ArrayLen>() {
    let n = S::USIZE;
    let expected: Vec<T> = (0..n).map(T::make).collect();
    let a: Array<T, S> = Array::from_fn(T::make);

    assert_eq!(a.as_slice(), &expected[..]);
    assert_eq!((a.len(), Array::<T, S>::LEN), (n, n));
    assert_eq!(a.as_ptr() as usize % align_of::<T>(), 0);
    assert_eq!(format!("{a:?}"), format!("{expected:?}"));
    assert_eq!(hash(&a), hash(&expected[..]));

    // Clones and conversions.
    assert_eq!(a.clone(), a);
    let mut b: Array<T, S> = Array::from_fn(|_| T::make(0));
    b.clone_from(&a);
    assert_eq!(b, a);
    assert_eq!(a.each_ref().map(T::clone), a);
    assert_eq!(a.clone().map(|x| x), a);
    assert_eq!(a.clone().zip(a.clone()).map(|(x, _)| x), a);
    assert_eq!(
        Array::<T, S>::try_from_iter(expected.clone()).as_ref(),
        Ok(&a)
    );
    assert_eq!(<&Array<T, S>>::try_from(&expected[..]).ok(), Some(&a));
    assert_eq!(
        Array::<T, S>::try_from(&expected[..]).ok().as_ref(),
        Some(&a)
    );

    // Iteration in both directions.
    assert_eq!(a.clone().into_iter().collect::<Vec<_>>(), expected);
    let reversed: Vec<T> = expected.iter().rev().cloned().collect();
    assert_eq!(a.clone().into_iter().rev().collect::<Vec<_>>(), reversed);
    assert_eq!(
        a.iter().collect::<Vec<_>>(),
        expected.iter().collect::<Vec<_>>()
    );

    // Writes through mutable views reach the array.
    let mut m = a.clone();
    for (i, x) in m.iter_mut().enumerate() {
        *x = T::make(n - 1 - i);
    }
    for x in m.each_mut() {
        *x = x.clone();
    }
    assert_eq!(m.as_slice(), &reversed[..]);

    // Casting to the flat size and back keeps the elements.
    let proof = SameLen::<S, F>::try_new().expect("F has the length of S");
    assert_eq!(a.cast_ref(proof).as_slice(), a.as_slice());
    let flat: Array<T, F> = a.clone().cast(proof);
    assert_eq!(flat.as_slice(), &expected[..]);
    let Ok(back) = flat.try_cast::<S>() else {
        panic!("try_cast back to the original size failed");
    };
    assert_eq!(back, a);

    // Views of the empty prefix and suffix, which exist for every size.
    assert!(a.prefix_ref(AtMost::<Len<0>, S>::zero()).is_empty());
    assert_eq!(a.split_suffix(AtMost::<Len<0>, S>::zero()).0, &expected[..]);
}

/// Injects a panic at every index of every operation that creates elements,
/// and checks that every element is dropped exactly once.
fn check_panics<S: ArrayLen>() {
    let n = S::USIZE;
    let base = live();
    let new = || Array::<Tracked, S>::from_fn(Tracked::new);

    for k in 0..=n {
        let result = with_fuse(k, new);
        assert_eq!(result.is_some(), k == n, "from_fn, panic at {k}");
        drop(result);
        assert_eq!(live(), base, "from_fn, panic at {k}");

        let result: Result<Array<Tracked, S>, usize> =
            Array::try_from_fn(|i| if i == k { Err(i) } else { Ok(Tracked::new(i)) });
        assert_eq!(result.is_ok(), k == n, "try_from_fn, error at {k}");
        drop(result);
        assert_eq!(live(), base, "try_from_fn, error at {k}");

        let a = new();
        let result = with_fuse(k, || a.clone());
        assert_eq!(result.is_some(), k == n, "clone, panic at {k}");
        drop((result, a));
        assert_eq!(live(), base, "clone, panic at {k}");

        let a = new();
        let result = with_fuse(k, || a.map(|t| Tracked::new(t.0)));
        assert_eq!(result.is_some(), k == n, "map, panic at {k}");
        drop(result);
        assert_eq!(live(), base, "map, panic at {k}");

        let fill = Tracked::new(0);
        let result = with_fuse(k, || {
            Array::<Tracked, S>::pad_from(Array::new([]), AtMost::zero(), fill)
        });
        assert_eq!(result.is_some(), k == n, "pad_from, panic at {k}");
        drop(result);
        assert_eq!(live(), base, "pad_from, panic at {k}");

        // Clone an iterator that has yielded from both ends.
        let mut iter = new().into_iter();
        drop((iter.next(), iter.next_back()));
        let remaining = iter.len();
        if k <= remaining {
            let result = with_fuse(k, || iter.clone());
            assert_eq!(
                result.is_some(),
                k == remaining,
                "IntoIter::clone, panic at {k}"
            );
            if let Some(copy) = result {
                assert_eq!(copy.as_slice(), iter.as_slice());
            }
        }
        drop(iter);
        assert_eq!(live(), base, "IntoIter::clone, panic at {k}");
    }
}

/// Checks that skipping and partially consuming an iterator drops every
/// element exactly once.
fn check_into_iter<S: ArrayLen>() {
    let n = S::USIZE;
    let base = live();
    let new = || Array::<Tracked, S>::from_fn(Tracked::new).into_iter();

    for skip in 0..=n + 1 {
        let mut iter = new();
        assert_eq!(iter.nth(skip).map(|t| t.0), (skip < n).then_some(skip));
        assert_eq!(iter.len(), n.saturating_sub(skip + 1));
        drop(iter);
        assert_eq!(live(), base, "nth({skip})");

        let mut iter = new();
        let back = n.checked_sub(skip + 1);
        assert_eq!(iter.nth_back(skip).map(|t| t.0), back);
        assert_eq!(iter.len(), n.saturating_sub(skip + 1));
        drop(iter);
        assert_eq!(live(), base, "nth_back({skip})");

        let mut iter = new();
        let front: Vec<_> = iter.by_ref().take(skip).collect();
        assert_eq!(front.len(), skip.min(n));
        assert_eq!(iter.as_slice().len(), n - skip.min(n));
        drop((front, iter));
        assert_eq!(live(), base, "take({skip})");
    }

    assert_eq!(new().count(), n);
    assert_eq!(new().last().map(|t| t.0), n.checked_sub(1));
    assert_eq!(live(), base);
}

macro_rules! size_matrix {
    ($($name:ident: $size:ty => $flat:ty,)*) => {$(
        mod $name {
            use super::*;

            #[test]
            fn values() {
                check_values::<(), $size, $flat>();
                check_values::<u8, $size, $flat>();
                check_values::<String, $size, $flat>();
                check_values::<Aligned, $size, $flat>();
                check_values::<Tracked, $size, $flat>();
                assert_eq!(live(), 0);
            }

            #[test]
            fn panics() {
                check_panics::<$size>();
            }

            #[test]
            fn into_iter() {
                check_into_iter::<$size>();
            }
        }
    )*};
}

size_matrix! {
    len_0: Len<0> => Len<0>,
    len_1: Len<1> => Len<1>,
    len_6: Len<6> => Len<6>,
    sum_empty_left: Sum<Len<0>, Len<6>> => Len<6>,
    sum_empty_right: Sum<Len<6>, Len<0>> => Len<6>,
    sum_empty_both: Sum<Len<0>, Len<0>> => Len<0>,
    sum_flat: Sum<Len<2>, Len<4>> => Len<6>,
    sum_nested_left: Sum<Sum<Len<1>, Len<2>>, Len<3>> => Len<6>,
    sum_nested_right: Sum<Len<1>, Sum<Len<2>, Len<3>>> => Len<6>,
    sum_deep: Sum<Sum<Sum<Len<1>, Len<1>>, Sum<Len<1>, Len<1>>>, Sum<Len<1>, Len<1>>> => Len<6>,
    prod_flat: Prod<Len<2>, Len<3>> => Len<6>,
    prod_flat_swapped: Prod<Len<3>, Len<2>> => Len<6>,
    prod_empty_outer: Prod<Len<0>, Len<5>> => Len<0>,
    prod_empty_inner: Prod<Len<5>, Len<0>> => Len<0>,
    prod_single: Prod<Len<1>, Len<1>> => Len<1>,
    prod_of_sum: Prod<Len<2>, Sum<Len<1>, Len<2>>> => Len<6>,
    sum_of_prods: Sum<Prod<Len<2>, Len<2>>, Prod<Len<1>, Len<2>>> => Len<6>,
    prod_nested: Prod<Prod<Len<1>, Len<2>>, Sum<Len<3>, Len<0>>> => Len<6>,
    prod_of_sum_outer: Prod<Sum<Len<1>, Len<1>>, Prod<Len<1>, Len<3>>> => Len<6>,
}
