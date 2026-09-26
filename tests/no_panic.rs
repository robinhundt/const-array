//! Tests that the checks in safe code which can't fail are optimized away.
//!
//! Many methods use checked operations whose checks can't fail, e.g.
//! `split_at(P::USIZE)` with an `AtMost<P, S>` proof, or `self[i]` in
//! `from_fn`. This keeps the unsafe code small, but relies on the optimizer to
//! remove the checks, as the lengths are constants after monomorphization.
//! The functions below are instantiated for concrete sizes and marked
//! `#[no_panic]`, which fails to link if a panic path remains in them.
//!
//! This only works with optimizations, so the tests only run without debug
//! assertions, in the `no-panic` profile:
//!
//!     cargo test --profile no-panic --test no_panic
//!
//! The profile is the release profile with a single codegen unit. With more,
//! a method is sometimes not inlined into the wrapper, and `no_panic` reports
//! any call that isn't inlined, whether or not it can panic.
//!
//! If linking fails, the error names the function, but not its module. The
//! wrappers are `#[inline(never)]`, so the object file shows which ones
//! reference the `no_panic` error symbol, and where the panic comes from.
//! Inspect them, e.g. with `cargo-show-asm`:
//!
//!     cargo asm --profile no-panic --test no_panic "no_panic::sum::from_fn"
#![cfg(not(debug_assertions))]

use std::hint::black_box;

use const_array::{Array, AtMost, Len, Prod, Sum, at_most};
use no_panic::no_panic;

/// Define `#[no_panic]` wrappers for the methods available for every size.
///
/// `$s` is the size to test, and `$p` a size at most as long, for the
/// methods that take an `AtMost` proof. The closures passed to the methods
/// can't panic themselves.
macro_rules! check_size {
    ($name:ident, $s:ty, $p:ty) => {
        mod $name {
            use super::*;

            type S = $s;
            type P = $p;
            const PROOF: AtMost<P, S> = at_most!(P, S);

            #[no_panic]
            #[inline(never)]
            pub fn from_fn() -> Array<u32, S> {
                Array::from_fn(|i| i as u32)
            }

            #[no_panic]
            #[inline(never)]
            pub fn clone(a: &Array<u32, S>) -> Array<u32, S> {
                a.clone()
            }

            #[no_panic]
            #[inline(never)]
            pub fn clone_from(a: &mut Array<u32, S>, b: &Array<u32, S>) {
                a.clone_from(b)
            }

            #[no_panic]
            #[inline(never)]
            pub fn map_ref(a: &Array<u32, S>) -> Array<u32, S> {
                a.map_ref(|x| x ^ 0x36)
            }

            #[no_panic]
            #[inline(never)]
            pub fn zip_with(a: &Array<u32, S>, b: &Array<u32, S>) -> Array<u32, S> {
                a.zip_with(b, |x, y| x.wrapping_add(*y))
            }

            #[no_panic]
            #[inline(never)]
            pub fn zip_mut_with(a: &mut Array<u32, S>, b: &Array<u32, S>) {
                a.zip_mut_with(b, |x, y| *x ^= y)
            }

            #[no_panic]
            #[inline(never)]
            pub fn each_ref(a: &Array<u32, S>) -> Array<&u32, S> {
                a.each_ref()
            }

            #[no_panic]
            #[inline(never)]
            pub fn truncate(a: Array<u32, S>) -> Array<u32, P> {
                a.truncate(PROOF)
            }

            #[no_panic]
            #[inline(never)]
            pub fn prefix_ref(a: &Array<u32, S>) -> &Array<u32, P> {
                a.prefix_ref(PROOF)
            }

            #[no_panic]
            #[inline(never)]
            pub fn split_prefix_mut(a: &mut Array<u32, S>) -> (&mut Array<u32, P>, &mut [u32]) {
                a.split_prefix_mut(PROOF)
            }

            #[no_panic]
            #[inline(never)]
            pub fn suffix_ref(a: &Array<u32, S>) -> &Array<u32, P> {
                a.suffix_ref(PROOF)
            }

            #[no_panic]
            #[inline(never)]
            pub fn split_suffix_mut(a: &mut Array<u32, S>) -> (&mut [u32], &mut Array<u32, P>) {
                a.split_suffix_mut(PROOF)
            }

            #[no_panic]
            #[inline(never)]
            pub fn try_from_slice(v: &[u32]) -> Option<&Array<u32, S>> {
                v.try_into().ok()
            }

            #[no_panic]
            #[inline(never)]
            pub fn slice_as_chunks(v: &[u32]) -> (&[Array<u32, S>], &[u32]) {
                Array::slice_as_chunks(v)
            }

            #[no_panic]
            #[inline(never)]
            pub fn concat(a: Array<u32, S>, b: Array<u32, P>) -> Array<u32, Sum<S, P>> {
                a.concat(b)
            }

            #[test]
            fn no_panic() {
                let a = black_box(from_fn());
                let mut b = black_box(clone(&a));
                clone_from(&mut b, &a);
                black_box(map_ref(&a));
                black_box(zip_with(&a, &b));
                zip_mut_with(&mut b, &a);
                black_box(each_ref(&a));
                black_box(truncate(a.clone()));
                black_box(prefix_ref(&a));
                black_box(split_prefix_mut(&mut b));
                black_box(suffix_ref(&a));
                black_box(split_suffix_mut(&mut b));
                black_box(try_from_slice(&a).unwrap());
                black_box(slice_as_chunks(&a));
                black_box(concat(a.clone(), Array::default()));
            }
        }
    };
}

// Plain lengths, from fully unrolled loops to ones that aren't unrolled.
check_size!(len_4, Len<4>, Len<3>);
check_size!(len_32, Len<32>, Len<16>);
check_size!(len_1024, Len<1024>, Len<1000>);
// Nested sizes, whose array types are built from `Concat` and `Repeat`.
check_size!(sum, Sum<Len<5>, Sum<Len<3>, Len<25>>>, Len<7>);
check_size!(prod, Prod<Len<16>, Len<4>>, Sum<Len<3>, Len<8>>);
check_size!(prod_large, Prod<Len<3>, Prod<Len<16>, Len<64>>>, Len<1000>);

/// Define `#[no_panic]` wrappers for the methods that consume an
/// [`IntoIter`](const_array::IntoIter).
///
/// They are not checked for large sizes: once the loop isn't unrolled, the
/// bounds check of `self.data[range]` in `IntoIter::drop_range`, which drops
/// the elements not yet yielded, remains. This is the case for `pad_from` with
/// `Len<1024>`, and for all three with `Prod<Len<3>, Prod<Len<16>, Len<64>>>`.
///
/// `try_from_fn` and `try_from_iter` are not checked at all, because the
/// `unreachable!` in `from_exact_iter`, which they call with a flattened
/// iterator of `Option`s, is never optimized away.
macro_rules! check_into_iter {
    ($name:ident, $s:ty, $p:ty) => {
        mod $name {
            use super::*;

            type S = $s;
            type P = $p;
            const PROOF: AtMost<P, S> = at_most!(P, S);

            #[no_panic]
            #[inline(never)]
            pub fn map(a: Array<u32, S>) -> Array<u64, S> {
                a.map(u64::from)
            }

            #[no_panic]
            #[inline(never)]
            pub fn zip(a: Array<u32, S>, b: Array<u8, S>) -> Array<(u32, u8), S> {
                a.zip(b)
            }

            #[no_panic]
            #[inline(never)]
            pub fn pad_from(prefix: Array<u32, P>) -> Array<u32, S> {
                Array::pad_from(prefix, PROOF, 0)
            }

            #[test]
            fn no_panic() {
                let a = black_box(Array::<u32, S>::from_fn(|i| i as u32));
                black_box(map(a.clone()));
                black_box(zip(a, Array::from_fn(|i| i as u8)));
                black_box(pad_from(Array::from_fn(|i| i as u32)));
            }
        }
    };
}

check_into_iter!(into_iter_len_4, Len<4>, Len<3>);
check_into_iter!(into_iter_len_32, Len<32>, Len<16>);
check_into_iter!(into_iter_sum, Sum<Len<5>, Sum<Len<3>, Len<25>>>, Len<7>);
check_into_iter!(into_iter_prod, Prod<Len<16>, Len<4>>, Sum<Len<3>, Len<8>>);

/// Define `#[no_panic]` wrappers for the methods of an array of size
/// `Sum<A, B>` and `Prod<A, B>`.
macro_rules! check_parts {
    ($name:ident, $a:ty, $b:ty) => {
        mod $name {
            use super::*;

            type A = $a;
            type B = $b;

            #[no_panic]
            #[inline(never)]
            pub fn split_ref(a: &Array<u32, Sum<A, B>>) -> (&Array<u32, A>, &Array<u32, B>) {
                a.split_ref()
            }

            #[no_panic]
            #[inline(never)]
            pub fn split_mut(
                a: &mut Array<u32, Sum<A, B>>,
            ) -> (&mut Array<u32, A>, &mut Array<u32, B>) {
                a.split_mut()
            }

            #[no_panic]
            #[inline(never)]
            pub fn parts(a: Array<u32, Sum<A, B>>) -> (Array<u32, A>, Array<u32, B>) {
                a.parts()
            }

            #[no_panic]
            #[inline(never)]
            pub fn as_chunks(a: &Array<u32, Prod<A, B>>) -> &Array<Array<u32, B>, A> {
                a.as_chunks()
            }

            #[no_panic]
            #[inline(never)]
            pub fn as_chunks_mut(a: &mut Array<u32, Prod<A, B>>) -> &mut Array<Array<u32, B>, A> {
                a.as_chunks_mut()
            }

            #[test]
            fn no_panic() {
                let mut sum = black_box(Array::<u32, Sum<A, B>>::from_fn(|i| i as u32));
                black_box(split_ref(&sum));
                black_box(split_mut(&mut sum));
                black_box(parts(sum));
                let mut prod = black_box(Array::<u32, Prod<A, B>>::from_fn(|i| i as u32));
                black_box(as_chunks(&prod));
                black_box(as_chunks_mut(&mut prod));
            }
        }
    };
}

check_parts!(parts_small, Len<3>, Len<5>);
check_parts!(parts_large, Len<64>, Prod<Len<16>, Len<3>>);

/// Define `#[no_panic]` wrappers for the conversions of plain arrays.
macro_rules! check_len {
    ($name:ident, $n:literal) => {
        mod $name {
            use super::*;

            #[no_panic]
            #[inline(never)]
            pub fn from_ref(a: &[u32; $n]) -> &Array<u32, Len<$n>> {
                Array::from_ref(a)
            }

            #[no_panic]
            #[inline(never)]
            pub fn from_mut(a: &mut [u32; $n]) -> &mut Array<u32, Len<$n>> {
                Array::from_mut(a)
            }

            #[test]
            fn no_panic() {
                let mut a = black_box([7; $n]);
                black_box(from_ref(&a));
                black_box(from_mut(&mut a));
            }
        }
    };
}

check_len!(plain_4, 4);
check_len!(plain_1024, 1024);
