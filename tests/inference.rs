//! Type-inference focused tests.
//!
//! These exercise the ergonomics of the public API: whether the compiler can
//! infer element types, sizes, and sub-array types from the surrounding
//! context (binding annotations, turbofish, `From`/`TryFrom`, `Deref`, and
//! `parts`). They are deliberately light on runtime assertions -- the point is
//! that the code type-checks -- but they still run cleanly under miri.

use std::panic::{RefUnwindSafe, UnwindSafe};

use const_array::{
    Array, ArrayLen, AtLeast, AtMost, Len, Prod, SameLen, Sum, at_least, at_most, same_len,
};

// Readable size aliases.
type S3 = Len<3>;
type S6 = Sum<Len<3>, Len<3>>;
type S7 = Sum<Len<1>, Sum<Len<2>, Len<4>>>;

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
    // `From<[T; N]> for Array<T, Len<N>>` drives inference in both directions.
    let a: Array<i32, Len<3>> = [10, 20, 30].into();
    assert_eq!(&*a, &[10, 20, 30]);

    let b = Array::from([1, 2, 3]);
    let _: Array<i32, Len<3>> = b;
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
    let arr: Array<u8, Sum<Len<1>, Len<3>>> = data[..].try_into().unwrap();
    assert_eq!(&*arr, &[1, 2, 3, 4]);
}

#[test]
fn parts_infers_subtypes() {
    let a: Array<i32, Sum<Len<2>, Len<3>>> = Array::from_fn(|i| i as i32);
    let (head, tail) = a.parts();
    // The split-out halves infer their sizes from the `Sum`.
    let _: Array<i32, Len<2>> = head;
    let _: Array<i32, Len<3>> = tail;
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
    // Requires `Array<i32, Sum<Len<1>, Len<2>>>: Copy`, which needs the
    // concrete `Concat<[i32; 1], [i32; 2]>: Copy`.
    let a: Array<i32, Sum<Len<1>, Len<2>>> = Array::from_fn(|i| i as i32);
    let b = a;
    let c = a; // still usable => `a` was copied, not moved
    assert_eq!(b.as_slice(), c.as_slice());
}

#[test]
fn concat_infers_sum_size() {
    let a = Array::from([1, 2]);
    let b = Array::from([3, 4, 5]);
    let c = Array::concat(a, b);
    let _: &Array<i32, Sum<Len<2>, Len<3>>> = &c;
    assert_eq!(&*c, &[1, 2, 3, 4, 5]);
    assert_eq!(c.parts(), (a, b));
}

#[test]
fn split_ref_infers_subtypes() {
    let a: Array<i32, S7> = Array::from_fn(|i| i as i32);
    let (x, rest) = a.split_ref();
    let (y, z) = rest.split_ref();
    let _: &Array<i32, Len<1>> = x;
    let _: &Array<i32, Len<2>> = y;
    let _: &Array<i32, Len<4>> = z;
    assert_eq!(&**z, &[3, 4, 5, 6]);
}

#[test]
fn cast_target_from_proof() {
    // The target size is inferred from the `same_len!` proof.
    let a: Array<u8, Len<6>> = Array::from_fn(|i| i as u8);
    let (x, y) = a.cast(same_len!(Len<6>, S6)).parts();
    assert_eq!(<[u8; 3]>::from(x), [0, 1, 2]);
    assert_eq!(<[u8; 3]>::from(y), [3, 4, 5]);
}

#[test]
fn try_cast_target_from_annotation() {
    let a: Array<u8, S6> = Array::from_fn(|i| i as u8);
    let b: Result<Array<u8, Len<6>>, _> = a.try_cast();
    assert!(b.is_ok());
    let c: Result<Array<u8, Len<7>>, _> = a.try_cast();
    assert_eq!(c.unwrap_err(), a);
}

// Generic code takes the proof as a parameter, so the requirement is part of
// its signature.
fn halves<T, S: ArrayLen>(a: Array<T, S>, proof: SameLen<S, S6>) -> (Array<T, S3>, Array<T, S3>) {
    a.cast(proof).parts()
}

#[test]
fn generic_fn_takes_proof() {
    let a: Array<u8, Len<6>> = Array::from_fn(|i| i as u8);
    let (x, y) = halves(a, same_len!(Len<6>, S6));
    assert_eq!((&*x, &*y), (&[0, 1, 2][..], &[3, 4, 5][..]));
}

#[test]
fn proof_combinators() {
    let p: SameLen<Len<6>, S6> = same_len!(Len<6>, S6);
    let _: SameLen<S6, Len<6>> = p.symm();
    let _: SameLen<S6, S6> = SameLen::refl();
    assert!(SameLen::<Len<6>, S7>::try_new().is_none());
}

#[test]
fn zip_with_keeps_the_size() {
    let a: Array<u8, S6> = Array::from_fn(|i| i as u8);
    let b: Array<u8, S6> = Array::from_fn(|_| 0xff);
    let x = a.zip_with(&b, |x, y| x ^ y);
    let _: &Array<u8, S6> = &x;
    assert_eq!(&*x, &[0xff, 0xfe, 0xfd, 0xfc, 0xfb, 0xfa]);
    let sums = a.zip_with(&b, |&x, &y| u16::from(x) + u16::from(y));
    assert_eq!(sums[5], 260);
}

#[test]
fn map_changes_element_type() {
    let a: Array<u32, S3> = Array::from([1, 2, 3]);
    let b: Array<[u8; 4], S3> = a.map(u32::to_be_bytes);
    assert_eq!(b[2], [0, 0, 0, 3]);
}

#[test]
fn std_traits_hold_in_generic_code() {
    fn assert_traits<X: Send + Sync + Clone + core::fmt::Debug + Eq + Ord + core::hash::Hash>() {}
    fn generic<T: Send + Sync + Clone + core::fmt::Debug + Ord + core::hash::Hash, S: ArrayLen>() {
        assert_traits::<Array<T, S>>();
    }
    generic::<u8, S7>();

    fn assert_auto<X: Send + Sync + Unpin + UnwindSafe + RefUnwindSafe>() {}
    fn generic_auto<T: Send + Sync + Unpin + UnwindSafe + RefUnwindSafe, S: ArrayLen>() {
        assert_auto::<Array<T, S>>();
        assert_auto::<const_array::IntoIter<T, S>>();
    }
    generic_auto::<u8, S7>();
    let a: Array<i32, S3> = Array::from([1, 2, 3]);
    assert_eq!(format!("{a:?}"), "[1, 2, 3]");
    assert!(a < Array::from([1, 2, 4]));
}

#[test]
fn iterate_by_reference() {
    let mut a: Array<i32, S6> = Array::from_fn(|i| i as i32);
    for x in &mut a {
        *x += 1;
    }
    let total: i32 = (&a).into_iter().sum();
    assert_eq!(total, 21);
}

#[test]
fn map_ref_leaves_array_usable() {
    // Generic over the size, so `Array` is not `Copy` here.
    fn pads<S: ArrayLen>(key: &Array<u8, S>) -> (Array<u8, S>, Array<u8, S>) {
        (key.map_ref(|b| b ^ 0x36), key.map_ref(|b| b ^ 0x5c))
    }
    let key: Array<u8, S3> = Array::from([0, 1, 2]);
    let (ipad, opad) = pads(&key);
    assert_eq!(&*ipad, &[0x36, 0x37, 0x34]);
    assert_eq!(&*opad, &[0x5c, 0x5d, 0x5e]);

    let names: Array<String, S3> = Array::from_fn(|i| i.to_string());
    let lens: Array<usize, S3> = names.map_ref(String::len);
    assert_eq!(&*lens, &[1, 1, 1]);
    assert_eq!(names[2], "2");
}

#[test]
fn zip_mut_with_updates_every_element() {
    // A `Sum` size, so the elements span both halves of the `Concat`.
    let mut a: Array<u8, S6> = Array::from_fn(|i| i as u8);
    let keystream: Array<u8, S6> = Array::from_fn(|_| 0xff);
    a.zip_mut_with(&keystream, |x, k| *x ^= k);
    assert_eq!(&*a, &[0xff, 0xfe, 0xfd, 0xfc, 0xfb, 0xfa]);

    // Element types may differ.
    let mut counts: Array<usize, S3> = Array::default();
    let words: Array<&str, S3> = Array::from(["a", "bb", "ccc"]);
    counts.zip_mut_with(&words, |c, w| *c += w.len());
    assert_eq!(&*counts, &[1, 2, 3]);
}

#[test]
fn plain_array_reference_conversions() {
    let mut raw = [1u8, 2, 3];
    let a: &Array<u8, S3> = (&raw).into();
    let back: &[u8; 3] = a.into();
    assert_eq!(back, &[1, 2, 3]);
    let m: &mut Array<u8, S3> = (&mut raw).into();
    m.as_mut_array()[0] = 9;
    assert_eq!(raw, [9, 2, 3]);
}

#[test]
fn const_construction() {
    const PREFIX: Array<u8, Len<4>> = Array::new(*b"conn");
    const NONCE: Array<u8, Sum<Len<4>, Len<8>>> = Array::concat(PREFIX, Array::new([7; 8]));
    const FLAT: Array<u8, Len<12>> = NONCE.cast(same_len!(Sum<Len<4>, Len<8>>, Len<12>));
    const COUNTER: Array<u8, Len<8>> = NONCE.parts().1;
    const BYTES: [u8; 12] = FLAT.into_array();
    assert_eq!(&BYTES[..4], b"conn");
    assert_eq!(COUNTER.as_array(), &[7; 8]);
    assert_eq!(NONCE.split_ref().0, &PREFIX);
}

#[test]
fn cast_checked_generic() {
    fn rotate<T, A: ArrayLen, B: ArrayLen, C: ArrayLen>(
        a: Array<T, Sum<Sum<A, B>, C>>,
    ) -> Array<T, Sum<A, Sum<B, C>>> {
        a.cast_checked()
    }
    let a: Array<u8, Sum<Sum<Len<1>, Len<2>>, Len<3>>> = Array::from_fn(|i| i as u8);
    let (x, rest) = rotate(a).parts();
    let (y, z) = rest.parts();
    assert_eq!((&*x, &*y, &*z), (&[0][..], &[1, 2][..], &[3, 4, 5][..]));
    let _: SameLen<Len<6>, S6> = SameLen::checked();
}

#[test]
fn prod_sizes_are_copy_and_castable() {
    let a: Array<i32, Prod<Len<2>, Len<3>>> = Array::from_fn(|i| i as i32);
    let b = a;
    assert_eq!(a.as_slice(), b.as_slice());
    let flat: Array<i32, Len<6>> = a.cast(same_len!(Prod<Len<2>, Len<3>>, Len<6>));
    assert_eq!(flat.as_array(), &[0, 1, 2, 3, 4, 5]);
    // The chunk sizes are inferred from the `Prod`.
    let rows = b.into_chunks();
    assert_eq!(rows[1].as_array(), &[3, 4, 5]);
}

#[test]
fn prod_in_const() {
    const ROW: Array<u8, Len<2>> = Array::new([1, 2]);
    const M: Array<u8, Prod<Len<2>, Len<2>>> = Array::from_chunks(Array::new([ROW, ROW]));
    assert_eq!(M.as_slice(), &[1, 2, 1, 2]);
}

#[test]
fn at_most_proof_combinators() {
    let p: AtMost<Len<2>, S3> = at_most!(Len<2>, S3);
    let q: AtMost<S3, S6> = at_most!(S3, S6);
    let _: AtMost<Len<2>, S6> = p.trans(q);
    let _: AtMost<S6, S6> = AtMost::refl();
    let _: AtMost<Len<6>, S6> = same_len!(Len<6>, S6).at_most();
    assert!(AtMost::<S7, S6>::try_new().is_none());
    assert!(AtMost::<S6, S7>::try_new().is_some());
}

#[test]
fn prefix_size_from_annotation_or_proof() {
    let a: Array<u8, S6> = Array::from_fn(|i| i as u8);
    // The prefix size is inferred from the proof...
    let (head, _) = a.split_prefix(at_most!(Len<2>, S6));
    let _: &Array<u8, Len<2>> = head;
    // ...or from the annotation, with a generic proof.
    let short: Array<u8, S3> = a.truncate(AtMost::checked());
    assert_eq!(&*short, &[0, 1, 2]);
    // A structural requirement in generic code: a part is at most as long as
    // the `Sum`.
    fn first_part<T, A: ArrayLen, B: ArrayLen>(a: &Array<T, Sum<A, B>>) -> &Array<T, A> {
        a.prefix_ref(AtMost::checked())
    }
    assert_eq!(first_part(&a).as_array(), &[0, 1, 2]);
}

#[test]
fn derives_on_types_generic_over_a_size() {
    // `ArrayLen` implies the std traits, so the derives' `S` bounds hold.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct Marker<S: ArrayLen>(S);

    // `Copy` is left out: `Array<u8, S>: Copy` can't be proven for a generic
    // `S` (see the docs of `Array`'s `Copy` impl).
    #[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    struct Key<S: ArrayLen> {
        bytes: Array<u8, S>,
        size: core::marker::PhantomData<S>,
    }

    fn check<S: ArrayLen>() {
        let m = Marker::<S>::default();
        let n = m;
        assert_eq!(m, n);
        assert_eq!(m.cmp(&n), core::cmp::Ordering::Equal);

        let k = Key::<S>::default();
        assert_eq!(k.clone(), k);
        assert!(k.bytes.iter().all(|&b| b == 0));
        let _ = format!("{m:?} {k:?}");
    }
    check::<S3>();
    check::<S7>();
    check::<Prod<Len<2>, S6>>();
}

#[test]
fn concat_as_method() {
    let a = Array::from([1u8, 2]);
    let b = Array::from([3u8]);
    let c = Array::from([4u8, 5, 6]);
    let abc = a.concat(b).concat(c);
    let _: &Array<u8, Sum<Sum<Len<2>, Len<1>>, Len<3>>> = &abc;
    assert_eq!(abc, Array::from_fn(|i| i as u8 + 1));
    // The associated function form still works.
    let _: Array<u8, Sum<Len<2>, Len<1>>> = Array::concat(a, b);
    // And in `const` code.
    const AB: Array<u8, Sum<Len<1>, Len<2>>> = Array::new([1]).concat(Array::new([2, 3]));
    assert_eq!(AB.as_slice(), &[1, 2, 3]);
}

#[test]
fn compare_with_plain_arrays() {
    let a = Array::from([1, 2, 3]);
    assert_eq!(a, [1, 2, 3]);
    assert_eq!([1, 2, 3], a);
    assert_ne!(a, [1, 2, 4]);
    // Element types only need to be comparable, like for plain arrays.
    let b: Array<&str, Len<2>> = Array::from(["a", "b"]);
    assert!(b == ["a", "b"]);
}

#[test]
fn try_from_slice_error_is_an_error() {
    let err = <&Array<u8, S3>>::try_from(&[1u8, 2][..]).unwrap_err();
    assert_eq!(err.to_string(), "could not convert slice to array");
    let boxed: Box<dyn std::error::Error> = Box::new(err);
    assert_eq!(boxed.to_string(), "could not convert slice to array");
}

/// The proof macros accept concrete sizes inside generic code, and `Self` in
/// impls for concrete types, also inside methods.
#[test]
fn proof_macros_in_generic_code_and_impls() {
    fn generic<T: Default>(a: Array<T, Len<6>>) -> (Array<T, Len<2>>, Array<T, Len<4>>) {
        let _: AtMost<Len<2>, Len<6>> = at_most!(Len<2>, Len<6>);
        a.cast(same_len!(Len<6>, Sum<Len<2>, Len<4>>)).parts()
    }
    let (x, y) = generic(Array::from([1, 2, 3, 4, 5, 6]));
    assert_eq!((x[1], y[0]), (2, 3));

    trait Block {
        type Size: ArrayLen;
        const HALVES: SameLen<Self::Size, Sum<Len<8>, Len<8>>>;
        fn halves(block: &Array<u8, Self::Size>) -> (&Array<u8, Len<8>>, &Array<u8, Len<8>>);
    }

    struct Cipher;

    impl Block for Cipher {
        type Size = Len<16>;
        const HALVES: SameLen<Self::Size, Sum<Len<8>, Len<8>>> =
            same_len!(Self::Size, Sum<Len<8>, Len<8>>);
        fn halves(block: &Array<u8, Self::Size>) -> (&Array<u8, Len<8>>, &Array<u8, Len<8>>) {
            let _: AtMost<Len<8>, Self::Size> = at_most!(Len<8>, Self::Size);
            block
                .cast_ref(same_len!(Self::Size, Sum<Len<8>, Len<8>>))
                .split_ref()
        }
    }

    let block = Array::from_fn(|i| i as u8);
    let (lo, hi) = Cipher::halves(&block);
    assert_eq!((lo[0], hi[0]), (0, 8));
    let _ = Cipher::HALVES;
}

#[test]
fn len_constant_needs_no_value() {
    const N: usize = Array::<u8, S7>::LEN;
    assert_eq!(N, 7);
    assert_eq!(Array::<(), Prod<Len<3>, S6>>::LEN, 18);
    assert_eq!(Array::<String, Len<0>>::LEN, 0);
}

#[test]
fn at_least_is_at_most_swapped() {
    let proof: AtLeast<S7, Len<3>> = at_least!(S7, Len<3>);
    let a: Array<u8, S7> = Array::from_fn(|i| i as u8);
    assert_eq!(a.prefix_ref(proof).as_slice(), &[0, 1, 2]);
    let _: AtLeast<S3, S3> = AtLeast::refl();
    assert!(AtLeast::<Len<2>, S3>::try_new().is_none());
}
