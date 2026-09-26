//! Tests that the checks in safe code which can't fail are optimized away.
//!
//! Many methods use checked operations whose checks can't fail, e.g.
//! `split_at(P::USIZE)` with an `AtMost<P, S>` proof, or `self[i]` in
//! `from_fn`. This keeps the unsafe code small, but relies on the optimizer to
//! remove the checks, as the lengths are constants after monomorphization.
//! The functions below are instantiated for concrete sizes and marked
//! `#[no_panic]`, which fails to link if a panic path remains in them.
//!
//! This only works with optimizations and depends on the compiler's inlining
//! decisions, so the tests only run when enabled with the `no_panic_tests`
//! cfg, in the `no-panic` profile:
//!
//! ```sh
//! RUSTFLAGS="--cfg no_panic_tests" cargo test --profile no-panic --test no_panic
//! ```
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
//! ```sh
//! RUSTFLAGS="--cfg no_panic_tests" cargo asm --profile no-panic --test no_panic \
//!     "no_panic::sum::from_fn"
//! ```
#![cfg(no_panic_tests)]

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
            pub fn each_mut(a: &mut Array<u32, S>) -> Array<&mut u32, S> {
                a.each_mut()
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

            #[no_panic]
            #[inline(never)]
            pub fn try_from_fn() -> Result<Array<u32, S>, ()> {
                Array::try_from_fn(|i| if i < 1 << 20 { Ok(i as u32) } else { Err(()) })
            }

            #[no_panic]
            #[inline(never)]
            pub fn try_from_iter(v: &[u32]) -> Option<Array<u32, S>> {
                Array::try_from_iter(v.iter().copied()).ok()
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
                black_box(each_mut(&mut b));
                black_box(truncate(a.clone()));
                black_box(prefix_ref(&a));
                black_box(split_prefix_mut(&mut b));
                black_box(suffix_ref(&a));
                black_box(split_suffix_mut(&mut b));
                black_box(map(a.clone()));
                black_box(zip(a.clone(), Array::from_fn(|i| i as u8)));
                black_box(pad_from(Array::from_fn(|i| i as u32)));
                black_box(try_from_fn().unwrap());
                black_box(try_from_iter(&a).unwrap());
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
