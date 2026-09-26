//! The [`Array`] type and its operations.

use core::{
    error::Error,
    fmt,
    mem::{self, ManuallyDrop},
    panic::{RefUnwindSafe, UnwindSafe},
    ptr, slice,
};

use crate::{
    ArrayLen, AtMost, Len, Prod, SameLen, Sum,
    size::{self, ArrayType, Concat},
};

/// An array of `S::USIZE` elements of type `T`, laid out as `[T; S::USIZE]`.
///
/// The size `S` is a [`Len`], [`Sum`] or [`Prod`]. Its structure determines
/// which operations are available without a cast, e.g. [`Array::split_ref`]
/// for a [`Sum`]. See the [crate docs](crate) for an overview.
// `Array` is `repr(transparent)` over `S::ArrayType<T>`, so by the `ArrayLen`
// and `ArrayType` invariants, `Array<T, S>` is laid out as `[T; S::USIZE]` for
// every `T` and `S`. The unsafe code in this crate relies on this.
#[repr(transparent)]
pub struct Array<T, S: ArrayLen>(pub(crate) S::ArrayType<T>);

impl<T: Clone, S: ArrayLen> Clone for Array<T, S> {
    fn clone(&self) -> Self {
        Self(self.0.clone_array())
    }

    fn clone_from(&mut self, source: &Self) {
        self.as_mut_slice().clone_from_slice(source);
    }
}

/// [`Array`] is [`Copy`] for every concrete [`ArrayLen`] if `T: Copy`.
///
/// In code that is generic over the size, the compiler cannot infer this.
/// There, prefer [`Clone::clone`], which is just as fast: it delegates to the
/// standard library's `Clone` for arrays, which is specialized to a plain copy
/// for `Copy` types. If you need actual `Copy` semantics, add the bound
/// `S: ArrayLen<ArrayType<T>: Copy>`:
///
/// ```
/// use const_array::{Array, ArrayLen};
///
/// fn duplicate<T: Copy, S: ArrayLen<ArrayType<T>: Copy>>(
///     a: &Array<T, S>,
/// ) -> (Array<T, S>, Array<T, S>) {
///     (*a, *a)
/// }
/// ```
impl<T: Copy, S: ArrayLen<ArrayType<T>: Copy>> Copy for Array<T, S> {}

// The compiler cannot infer auto traits for `Array<T, S>` in code that is
// generic over `S`, because the field's type is a projection. By the
// `ArrayType` invariant, `Array<T, S>` consists of nothing but values of `T`,
// so it has these traits exactly when `T` does.
// SAFETY: See above.
unsafe impl<T: Send, S: ArrayLen> Send for Array<T, S> {}
// SAFETY: See above.
unsafe impl<T: Sync, S: ArrayLen> Sync for Array<T, S> {}
impl<T: Unpin, S: ArrayLen> Unpin for Array<T, S> {}
impl<T: UnwindSafe, S: ArrayLen> UnwindSafe for Array<T, S> {}
impl<T: RefUnwindSafe, S: ArrayLen> RefUnwindSafe for Array<T, S> {}

impl<T, S: ArrayLen> Array<T, S> {
    // Checks part of the layout that the `ArrayLen` and `ArrayType` invariants
    // guarantee. The traits are sealed, so this only fails if this crate has a
    // bug. It is a sanity check and not needed for soundness.
    pub(crate) const LAYOUT_OK: () = {
        assert!(<S::ArrayType<T> as ArrayType<T>>::LEN == S::USIZE);
        assert!(mem::align_of::<Self>() == mem::align_of::<T>());
        assert!(mem::size_of::<Self>() == mem::size_of::<T>() * S::USIZE);
    };

    /// The number of elements. Unlike [`len`](slice::len), it needs no value:
    ///
    /// ```
    /// use const_array::{Array, ArrayLen, Len, Prod};
    ///
    /// fn buffer_len<B: ArrayLen>() -> usize {
    ///     Array::<u8, Prod<Len<4>, B>>::LEN
    /// }
    ///
    /// assert_eq!(buffer_len::<Len<16>>(), 64);
    /// ```
    pub const LEN: usize = S::USIZE;

    /// View the [`Array`] as a slice.
    pub const fn as_slice(&self) -> &[T] {
        const { Self::LAYOUT_OK };
        size::as_slice(&self.0)
    }

    /// View the [`Array`] as a mutable slice.
    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        const { Self::LAYOUT_OK };
        size::as_mut_slice(&mut self.0)
    }

    /// View `slice` as `n` consecutive [`Array`]s, or return `None` if it
    /// does not hold exactly `n * S::USIZE` elements.
    ///
    /// This and [`Array::slice_as_arrays_mut`] are the only places that turn
    /// slices into [`Array`]s. Their callers are safe code, so a length that
    /// is wrong, e.g. due to a bug in a proof, results in a panic rather than
    /// undefined behavior. The lengths are constants in practice, so the
    /// checks are optimized away.
    const fn slice_as_arrays(slice: &[T], n: usize) -> Option<&[Self]> {
        const { Self::LAYOUT_OK };
        match n.checked_mul(S::USIZE) {
            Some(len) if len == slice.len() => {
                // SAFETY: `slice` holds `n * S::USIZE` elements, and `Self` is
                // laid out as `[T; S::USIZE]`, so they are `n` values of
                // `Self`. The result borrows from `slice`.
                Some(unsafe { slice::from_raw_parts(slice.as_ptr().cast(), n) })
            }
            _ => None,
        }
    }

    /// Mutable version of [`Array::slice_as_arrays`].
    const fn slice_as_arrays_mut(slice: &mut [T], n: usize) -> Option<&mut [Self]> {
        const { Self::LAYOUT_OK };
        match n.checked_mul(S::USIZE) {
            Some(len) if len == slice.len() => {
                // SAFETY: `slice` holds `n * S::USIZE` elements, and `Self` is
                // laid out as `[T; S::USIZE]`, so they are `n` values of
                // `Self`. The result mutably borrows from `slice`.
                Some(unsafe { slice::from_raw_parts_mut(slice.as_mut_ptr().cast(), n) })
            }
            _ => None,
        }
    }

    /// View `slice` as an [`Array`], or return `None` if it does not hold
    /// exactly `S::USIZE` elements.
    const fn from_slice(slice: &[T]) -> Option<&Self> {
        match Self::slice_as_arrays(slice, 1) {
            Some([array]) => Some(array),
            _ => None,
        }
    }

    /// View `slice` as a mutable [`Array`], or return `None` if it does not
    /// hold exactly `S::USIZE` elements.
    const fn from_mut_slice(slice: &mut [T]) -> Option<&mut Self> {
        match Self::slice_as_arrays_mut(slice, 1) {
            Some([array]) => Some(array),
            _ => None,
        }
    }

    /// Split a slice into [`Array`]s and the remaining elements. Like
    /// [`<[T]>::as_chunks`](slice::as_chunks), it fails to compile for a size
    /// of length 0.
    ///
    /// ```
    /// use const_array::{Array, Len};
    ///
    /// // E.g. the full blocks of a message, and the bytes left to buffer.
    /// let data = [0u8; 100];
    /// let (blocks, rest) = Array::<u8, Len<16>>::slice_as_chunks(&data);
    /// assert_eq!((blocks.len(), rest.len()), (6, 4));
    /// ```
    ///
    /// ```compile_fail
    /// use const_array::{Array, Len};
    ///
    /// let _ = Array::<u8, Len<0>>::slice_as_chunks(&[1, 2, 3]);
    /// ```
    pub const fn slice_as_chunks(slice: &[T]) -> (&[Self], &[T]) {
        const { assert!(S::USIZE != 0, "Array::slice_as_chunks: the chunk size is 0") };
        let len = slice.len() / S::USIZE;
        let (chunks, rest) = slice.split_at(len * S::USIZE);
        (Self::slice_as_arrays(chunks, len).unwrap(), rest)
    }

    /// Mutable version of [`Array::slice_as_chunks`].
    pub const fn slice_as_chunks_mut(slice: &mut [T]) -> (&mut [Self], &mut [T]) {
        const {
            assert!(
                S::USIZE != 0,
                "Array::slice_as_chunks_mut: the chunk size is 0"
            )
        };
        let len = slice.len() / S::USIZE;
        let (chunks, rest) = slice.split_at_mut(len * S::USIZE);
        (Self::slice_as_arrays_mut(chunks, len).unwrap(), rest)
    }

    /// Construct a new array from a function.
    ///
    /// The function is called with each index of the array in order.
    pub fn from_fn<F: FnMut(usize) -> T>(f: F) -> Array<T, S> {
        Array(S::ArrayType::build(f, 0))
    }

    /// Reinterpret the [`Array`] as an array of size `S2` with the same length.
    ///
    /// This does not move or copy individual elements. Together with
    /// [`Array::parts`], it can be used to split an array at an arbitrary
    /// point:
    ///
    /// ```
    /// use const_array::{same_len, Array, Len, Sum};
    ///
    /// let key: Array<u8, Len<32>> = Array::from_fn(|i| i as u8);
    /// let (enc_key, mac_key) = key.cast(same_len!(Len<32>, Sum<Len<16>, Len<16>>)).parts();
    /// assert_eq!(enc_key[0], 0);
    /// assert_eq!(mac_key[0], 16);
    /// ```
    pub const fn cast<S2: ArrayLen>(self, _proof: SameLen<S, S2>) -> Array<T, S2> {
        const { Self::LAYOUT_OK };
        const { Array::<T, S2>::LAYOUT_OK };
        // SAFETY: By `SameLen`'s invariant, `S::USIZE == S2::USIZE`, so
        // `Array<T, S>` and `Array<T, S2>` are both laid out as `[T;
        // S::USIZE]`.
        unsafe { transmute_layout(self) }
    }

    /// Reinterpret a reference to the [`Array`] as a reference to an array of
    /// size `S2` with the same length.
    pub const fn cast_ref<S2: ArrayLen>(&self, _proof: SameLen<S, S2>) -> &Array<T, S2> {
        // By `SameLen`'s invariant, `S::USIZE == S2::USIZE`, so this doesn't
        // panic.
        Array::from_slice(self.as_slice()).unwrap()
    }

    /// Reinterpret a mutable reference to the [`Array`] as a mutable reference
    /// to an array of size `S2` with the same length.
    pub const fn cast_mut<S2: ArrayLen>(&mut self, _proof: SameLen<S, S2>) -> &mut Array<T, S2> {
        // By `SameLen`'s invariant, `S::USIZE == S2::USIZE`, so this doesn't
        // panic.
        Array::from_mut_slice(self.as_mut_slice()).unwrap()
    }

    /// Reinterpret the [`Array`] as an array of size `S2` if both have the
    /// same length. Otherwise, the array is returned unchanged as the error.
    ///
    /// The check only involves constants and is optimized away. For concrete
    /// sizes, prefer [`Array::cast`] with [`same_len!`](crate::same_len!),
    /// which turns a length mismatch into a compile error.
    pub fn try_cast<S2: ArrayLen>(self) -> Result<Array<T, S2>, Self> {
        match SameLen::try_new() {
            Some(proof) => Ok(self.cast(proof)),
            None => Err(self),
        }
    }

    /// Reinterpret the [`Array`] as an array of size `S2` with the same
    /// length, failing to compile when monomorphized with sizes of different
    /// lengths.
    ///
    /// This is a shorthand for `self.cast(SameLen::checked())`. It is meant
    /// for generic code in which the lengths are equal for every instantiation
    /// the caller can choose. Note that a mismatch is **not** reported by
    /// `cargo check`, only by `cargo build`. See [`SameLen::checked`] for when
    /// to use it, and use [`Array::cast_ref`] or [`Array::cast_mut`] with
    /// [`SameLen::checked`] for references.
    pub const fn cast_checked<S2: ArrayLen>(self) -> Array<T, S2> {
        self.cast(SameLen::checked())
    }

    /// Keep the first `P::USIZE` elements and drop the rest.
    ///
    /// ```
    /// use const_array::{at_most, Array, Len};
    ///
    /// // E.g. a truncated MAC tag, like HMAC-SHA-256-128.
    /// let tag: Array<u8, Len<32>> = Array::from_fn(|i| i as u8);
    /// let short: Array<u8, Len<16>> = tag.truncate(at_most!(Len<16>, Len<32>));
    /// assert_eq!(short[15], 15);
    /// ```
    pub fn truncate<P: ArrayLen>(self, _proof: AtMost<P, S>) -> Array<T, P> {
        const { Self::LAYOUT_OK };
        const { Array::<T, P>::LAYOUT_OK };
        let mut me = ManuallyDrop::new(self);
        // SAFETY: By `AtMost`'s invariant, `P::USIZE <= S::USIZE`, so `Array<T,
        // P>` is laid out as the first `P::USIZE` elements of `self`.
        // `me` is never dropped, so the result takes ownership of them.
        let prefix = unsafe { ptr::read(ptr::from_ref(&me).cast::<Array<T, P>>()) };
        // SAFETY: The remaining elements were not read above, so `me` still
        // owns them, and they are dropped exactly once. If one of them
        // panics on drop, the others are still dropped, and so is
        // `prefix` during unwinding.
        unsafe { ptr::drop_in_place(&mut me.as_mut_slice()[P::USIZE..]) };
        prefix
    }

    /// View the first `P::USIZE` elements as an [`Array`].
    pub const fn prefix_ref<P: ArrayLen>(&self, proof: AtMost<P, S>) -> &Array<T, P> {
        self.split_prefix(proof).0
    }

    /// View the first `P::USIZE` elements as a mutable [`Array`].
    pub const fn prefix_mut<P: ArrayLen>(&mut self, proof: AtMost<P, S>) -> &mut Array<T, P> {
        self.split_prefix_mut(proof).0
    }

    /// Split into the first `P::USIZE` elements, as an [`Array`], and a slice
    /// of the rest.
    ///
    /// The rest is a slice, because its size `S - P` cannot be expressed as a
    /// type in generic code. If you need it as an [`Array`], name its size
    /// `R`, cast to [`Sum<P, R>`](Sum) with [`Array::cast_ref`] and split
    /// with [`Array::split_ref`] instead.
    ///
    /// ```
    /// use const_array::{at_most, Array, Len};
    ///
    /// // A record whose first 5 bytes are a header.
    /// let record: Array<u8, Len<32>> = Array::from_fn(|i| i as u8);
    /// let (header, payload) = record.split_prefix(at_most!(Len<5>, Len<32>));
    /// assert_eq!(header.as_array(), &[0, 1, 2, 3, 4]);
    /// assert_eq!(payload.len(), 27);
    /// ```
    pub const fn split_prefix<P: ArrayLen>(&self, _proof: AtMost<P, S>) -> (&Array<T, P>, &[T]) {
        // By `AtMost`'s invariant, `P::USIZE <= S::USIZE`, so this doesn't
        // panic.
        let (prefix, rest) = self.as_slice().split_at(P::USIZE);
        (Array::from_slice(prefix).unwrap(), rest)
    }

    /// Split into the first `P::USIZE` elements, as a mutable [`Array`], and a
    /// mutable slice of the rest.
    pub const fn split_prefix_mut<P: ArrayLen>(
        &mut self,
        _proof: AtMost<P, S>,
    ) -> (&mut Array<T, P>, &mut [T]) {
        // By `AtMost`'s invariant, `P::USIZE <= S::USIZE`, so this doesn't
        // panic.
        let (prefix, rest) = self.as_mut_slice().split_at_mut(P::USIZE);
        (Array::from_mut_slice(prefix).unwrap(), rest)
    }

    /// View the last `P::USIZE` elements as an [`Array`].
    ///
    /// ```
    /// use const_array::{at_most, Array, Len};
    ///
    /// // E.g. the 8 byte length field at the end of a padded hash block.
    /// let block: Array<u8, Len<64>> = Array::from_fn(|i| i as u8);
    /// let len_field: &Array<u8, Len<8>> = block.suffix_ref(at_most!(Len<8>, Len<64>));
    /// assert_eq!(len_field[0], 56);
    /// ```
    pub const fn suffix_ref<P: ArrayLen>(&self, proof: AtMost<P, S>) -> &Array<T, P> {
        self.split_suffix(proof).1
    }

    /// View the last `P::USIZE` elements as a mutable [`Array`].
    pub const fn suffix_mut<P: ArrayLen>(&mut self, proof: AtMost<P, S>) -> &mut Array<T, P> {
        self.split_suffix_mut(proof).1
    }

    /// Split into a slice of the rest and the last `P::USIZE` elements, as an
    /// [`Array`]. See [`Array::split_prefix`].
    pub const fn split_suffix<P: ArrayLen>(&self, _proof: AtMost<P, S>) -> (&[T], &Array<T, P>) {
        // By `AtMost`'s invariant, `P::USIZE <= S::USIZE`, so this neither
        // overflows nor panics.
        let (rest, suffix) = self.as_slice().split_at(S::USIZE - P::USIZE);
        (rest, Array::from_slice(suffix).unwrap())
    }

    /// Split into a mutable slice of the rest and the last `P::USIZE`
    /// elements, as a mutable [`Array`].
    pub const fn split_suffix_mut<P: ArrayLen>(
        &mut self,
        _proof: AtMost<P, S>,
    ) -> (&mut [T], &mut Array<T, P>) {
        // By `AtMost`'s invariant, `P::USIZE <= S::USIZE`, so this neither
        // overflows nor panics.
        let (rest, suffix) = self.as_mut_slice().split_at_mut(S::USIZE - P::USIZE);
        (rest, Array::from_mut_slice(suffix).unwrap())
    }

    /// Construct an array that starts with `prefix` and is filled up with
    /// clones of `fill`.
    ///
    /// ```
    /// use const_array::{at_most, Array, Len};
    ///
    /// // E.g. an HMAC key, zero-padded to the block size.
    /// let key: Array<u8, Len<32>> = Array::from([0xab; 32]);
    /// let block: Array<u8, Len<64>> = Array::pad_from(key, at_most!(Len<32>, Len<64>), 0);
    /// assert_eq!((block[31], block[32]), (0xab, 0));
    /// ```
    pub fn pad_from<P: ArrayLen>(prefix: Array<T, P>, _proof: AtMost<P, S>, fill: T) -> Self
    where
        T: Clone,
    {
        // By `AtMost`'s invariant, `P::USIZE <= S::USIZE`, so every element of
        // `prefix` is used.
        let mut prefix = prefix.into_iter();
        Self::from_fn(|_| prefix.next().unwrap_or_else(|| fill.clone()))
    }

    /// Construct a new array from a fallible function.
    ///
    /// The function is called with each index of the array in order, until it
    /// returns the first error. The elements built so far are then dropped
    /// and the error is returned.
    ///
    /// ```
    /// use const_array::{Array, Len};
    ///
    /// let hex = b"00ff10";
    /// let parse = |i: usize| {
    ///     let digits = core::str::from_utf8(&hex[2 * i..2 * i + 2]).unwrap();
    ///     u8::from_str_radix(digits, 16)
    /// };
    /// let bytes: Array<u8, Len<3>> = Array::try_from_fn(parse)?;
    /// assert_eq!(bytes, [0x00, 0xff, 0x10]);
    /// # Ok::<(), core::num::ParseIntError>(())
    /// ```
    pub fn try_from_fn<E, F: FnMut(usize) -> Result<T, E>>(mut f: F) -> Result<Self, E> {
        // Build the elements as `Option`s, so that no unsafe code is needed to
        // handle a partially built array.
        let mut error = None;
        let elements: Array<Option<T>, S> = Array::from_fn(|i| {
            if error.is_some() {
                return None;
            }
            f(i).map_err(|e| error = Some(e)).ok()
        });
        match error {
            Some(e) => Err(e),
            None => Ok(Self::from_exact_iter(elements.into_iter().flatten())),
        }
    }

    /// Construct a new array from an iterator that yields exactly `S::USIZE`
    /// elements.
    ///
    /// If it yields fewer or more elements, [`TryFromIterError`] is returned.
    /// The iterator is advanced at most `S::USIZE + 1` times.
    ///
    /// ```
    /// use const_array::{Array, Len};
    ///
    /// let squares: Array<u32, Len<4>> = Array::try_from_iter((0..4).map(|i| i * i)).unwrap();
    /// assert_eq!(squares, [0, 1, 4, 9]);
    /// assert!(Array::<u32, Len<4>>::try_from_iter(0..3).is_err());
    /// assert!(Array::<u32, Len<4>>::try_from_iter(0..5).is_err());
    /// ```
    pub fn try_from_iter<I: IntoIterator<Item = T>>(iter: I) -> Result<Self, TryFromIterError> {
        let mut iter = iter.into_iter();
        let array = Self::try_from_fn(|_| iter.next().ok_or(TryFromIterError(())))?;
        match iter.next() {
            Some(_) => Err(TryFromIterError(())),
            None => Ok(array),
        }
    }

    /// Construct a new array from an iterator that yields at least `S::USIZE`
    /// elements. Panics if it yields fewer.
    fn from_exact_iter<I: Iterator<Item = T>>(mut iter: I) -> Self {
        Self::from_fn(|_| match iter.next() {
            Some(x) => x,
            None => unreachable!("iterator is shorter than the array"),
        })
    }

    /// Apply `f` to each element, returning an array of the results.
    pub fn map<U, F: FnMut(T) -> U>(self, f: F) -> Array<U, S> {
        // `into_iter` has the length of the array, so this doesn't panic.
        Array::from_exact_iter(self.into_iter().map(f))
    }

    /// Borrow each element, returning an array of references.
    ///
    /// ```
    /// use const_array::{Array, Len};
    ///
    /// let names: Array<String, Len<2>> = Array::from(["a".to_owned(), "b".to_owned()]);
    /// let lens: Array<usize, Len<2>> = names.each_ref().map(|s| s.len());
    /// assert_eq!(lens, [1, 1]);
    /// ```
    pub fn each_ref(&self) -> Array<&T, S> {
        // `iter` has the length of the array, so this doesn't panic.
        Array::from_exact_iter(self.iter())
    }

    /// Mutably borrow each element, returning an array of mutable references.
    pub fn each_mut(&mut self) -> Array<&mut T, S> {
        // `iter_mut` has the length of the array, so this doesn't panic.
        Array::from_exact_iter(self.iter_mut())
    }

    /// Combine two arrays of the same size into an array of pairs.
    ///
    /// Unlike `a.into_iter().zip(b)`, this cannot silently truncate the
    /// result, as both arrays must have the same size.
    ///
    /// ```
    /// use const_array::{Array, Len};
    ///
    /// let keys = Array::from(["a", "b"]);
    /// let values = Array::from([1, 2]);
    /// let pairs: Array<(&str, i32), Len<2>> = keys.zip(values);
    /// assert_eq!(pairs, [("a", 1), ("b", 2)]);
    /// ```
    pub fn zip<U>(self, other: Array<U, S>) -> Array<(T, U), S> {
        // Both iterators have the length of the array, so this doesn't panic.
        Array::from_exact_iter(self.into_iter().zip(other))
    }

    /// Apply `f` to a reference to each element, returning an array of the
    /// results.
    ///
    /// Unlike [`Array::map`], this leaves the array usable. This matters in
    /// code that is generic over the size, where `Array` is not `Copy`:
    ///
    /// ```
    /// use const_array::{Array, ArrayLen};
    ///
    /// fn pads<S: ArrayLen>(key: &Array<u8, S>) -> (Array<u8, S>, Array<u8, S>) {
    ///     (key.map_ref(|b| b ^ 0x36), key.map_ref(|b| b ^ 0x5c))
    /// }
    /// ```
    pub fn map_ref<U, F: FnMut(&T) -> U>(&self, mut f: F) -> Array<U, S> {
        Array::from_fn(|i| f(&self[i]))
    }

    /// Combine the elements of two arrays of the same size with `f`.
    ///
    /// Unlike `a.iter().zip(b)`, this cannot silently truncate the result, as
    /// both arrays must have the same size.
    pub fn zip_with<U, V, F: FnMut(&T, &U) -> V>(
        &self,
        other: &Array<U, S>,
        mut f: F,
    ) -> Array<V, S> {
        Array::from_fn(|i| f(&self[i], &other[i]))
    }

    /// Update each element in place with the corresponding element of an
    /// array of the same size.
    ///
    /// Unlike `a.iter_mut().zip(b)`, this cannot silently skip elements, as
    /// both arrays must have the same size.
    ///
    /// ```
    /// use const_array::{Array, Len};
    ///
    /// let mut block: Array<u8, Len<4>> = Array::from([1, 2, 3, 4]);
    /// let keystream = Array::from([0xff; 4]);
    /// block.zip_mut_with(&keystream, |b, k| *b ^= k);
    /// assert_eq!(&*block, &[0xfe, 0xfd, 0xfc, 0xfb]);
    /// ```
    pub fn zip_mut_with<U, F: FnMut(&mut T, &U)>(&mut self, other: &Array<U, S>, mut f: F) {
        for (x, y) in self.iter_mut().zip(other.iter()) {
            f(x, y);
        }
    }

    /// Concatenate `self` and `other` into an [`Array`] of size
    /// [`Sum<S, B>`](Sum). This is the inverse of [`Array::parts`]. To get
    /// another size of the same length, such as a flat [`Len<N>`], cast the
    /// result with [`Array::cast`].
    ///
    /// ```
    /// use const_array::{Array, Len, Sum};
    ///
    /// let a = Array::from([1u8, 2]);
    /// let b = Array::from([3u8]);
    /// let c = Array::from([4u8, 5, 6]);
    /// let abc: Array<u8, Sum<Sum<Len<2>, Len<1>>, Len<3>>> = a.concat(b).concat(c);
    /// assert_eq!(abc.as_slice(), &[1, 2, 3, 4, 5, 6]);
    /// ```
    pub const fn concat<B: ArrayLen>(self, other: Array<T, B>) -> Array<T, Sum<S, B>> {
        const { Array::<T, Sum<S, B>>::LAYOUT_OK };
        // SAFETY: `Array<T, Sum<S, B>>` is `repr(transparent)` over
        // `Concat<S::ArrayType<T>, B::ArrayType<T>>`, and `Self` and
        // `Array<T, B>` are `repr(transparent)` over its fields. Unlike
        // `Array(Concat(self.0, other.0))`, this is allowed in a `const fn`.
        unsafe { transmute_layout(Concat(self, other)) }
    }

    /// Wrap a reference to the inner array type into an [`Array`].
    const fn wrap_ref(inner: &S::ArrayType<T>) -> &Self {
        // By the `ArrayLen` invariant, `inner` has `S::USIZE` elements, so this
        // doesn't panic.
        Self::from_slice(size::as_slice(inner)).unwrap()
    }

    /// Wrap a mutable reference to the inner array type into an [`Array`].
    const fn wrap_mut(inner: &mut S::ArrayType<T>) -> &mut Self {
        // By the `ArrayLen` invariant, `inner` has `S::USIZE` elements, so this
        // doesn't panic.
        Self::from_mut_slice(size::as_mut_slice(inner)).unwrap()
    }
}

/// Move `src` into a value of type `Dst`, without running its destructor.
///
/// Like [`mem::transmute`], but it also works for types whose size depends on
/// generic parameters.
///
/// # Safety
/// `Dst` must be laid out as `Src`, and any value of `Src` must be a valid
/// value of `Dst`. The result takes ownership of whatever `src` owns.
pub(crate) const unsafe fn transmute_layout<Src, Dst>(src: Src) -> Dst {
    // Not a `const` assertion, because callers like `Array::try_cast` may
    // instantiate this with mismatched types in branches that are never taken.
    debug_assert!(mem::size_of::<Src>() == mem::size_of::<Dst>());
    debug_assert!(mem::align_of::<Src>() == mem::align_of::<Dst>());
    let src = ManuallyDrop::new(src);
    // SAFETY: By the caller, `src` is also a valid `Dst`. It is never
    // dropped, so the result takes ownership.
    unsafe { ptr::read(ptr::from_ref(&src).cast()) }
}

/// Conversions between an [`Array`] of size [`Sum<A, B>`](Sum) and its parts
/// of sizes `A` and `B`. None of them move or copy individual elements.
///
/// To split an [`Array`] of another size, such as a flat [`Len<N>`], first
/// cast it to a [`Sum`] of the same length:
///
/// ```
/// use const_array::{same_len, Array, Len, Sum};
///
/// let sig: Array<u8, Len<64>> = Array::from_fn(|i| i as u8);
/// let (r, s) = sig.cast_ref(same_len!(Len<64>, Sum<Len<32>, Len<32>>)).split_ref();
/// assert_eq!((r[0], s[0]), (0, 32));
/// ```
impl<T, A: ArrayLen, B: ArrayLen> Array<T, Sum<A, B>> {
    /// Split a concatenated [`Array`] into its parts. This is the inverse of
    /// [`Array::concat`].
    pub const fn parts(self) -> (Array<T, A>, Array<T, B>) {
        const { Self::LAYOUT_OK };
        let me = ManuallyDrop::new(self);
        let concat: *const Concat<A::ArrayType<T>, B::ArrayType<T>> = ptr::from_ref(&me).cast();
        // SAFETY: `me` is laid out as `self`, which is `repr(transparent)` over
        // `Concat`, and `Array<T, A>` and `Array<T, B>` are `repr(transparent)`
        // over its fields. `me` is never dropped, so the result takes
        // ownership of the fields.
        // Unlike destructuring `self`, this is allowed in a `const fn`.
        unsafe {
            (
                ptr::read((&raw const (*concat).0).cast()),
                ptr::read((&raw const (*concat).1).cast()),
            )
        }
    }

    /// Split a reference to a concatenated [`Array`] into references to its
    /// parts.
    pub const fn split_ref(&self) -> (&Array<T, A>, &Array<T, B>) {
        let Concat(a, b) = &self.0;
        (Array::wrap_ref(a), Array::wrap_ref(b))
    }

    /// Split a mutable reference to a concatenated [`Array`] into mutable
    /// references to its parts.
    pub const fn split_mut(&mut self) -> (&mut Array<T, A>, &mut Array<T, B>) {
        let Concat(a, b) = &mut self.0;
        (Array::wrap_mut(a), Array::wrap_mut(b))
    }
}

/// Conversions between an [`Array`] of size [`Prod<A, B>`](Prod) and an
/// [`Array`] of `A` chunks with `B` elements each. Element `i` is element
/// `i % B::USIZE` of chunk `i / B::USIZE`. None of them move or copy
/// individual elements.
///
/// ```
/// use const_array::{Array, Len, Prod};
///
/// let matrix: Array<u8, Prod<Len<3>, Len<4>>> = Array::from_fn(|i| i as u8);
/// let rows = matrix.as_chunks();
/// assert_eq!(rows[1][2], matrix[6]);
/// assert_eq!(rows[2].as_slice(), &[8, 9, 10, 11]);
/// ```
///
/// To chunk an [`Array`] of another size, first cast it to a [`Prod`] of the
/// same length:
///
/// ```
/// use const_array::{same_len, Array, Len, Prod};
///
/// let buf: Array<u8, Len<64>> = Array::from_fn(|i| i as u8);
/// let blocks = buf.cast_ref(same_len!(Len<64>, Prod<Len<4>, Len<16>>)).as_chunks();
/// assert_eq!(blocks[1][0], 16);
/// ```
// `Array<T, Prod<A, B>>` is laid out as `[T; A::USIZE * B::USIZE]`.
// `Array<Array<T, B>, A>` is laid out as `[Array<T, B>; A::USIZE]`, which is
// `[[T; B::USIZE]; A::USIZE]`, so both types are laid out the same.
impl<T, A: ArrayLen, B: ArrayLen> Array<T, Prod<A, B>> {
    /// Flatten an [`Array`] of chunks. This is the inverse of
    /// [`Array::into_chunks`].
    pub const fn from_chunks(chunks: Array<Array<T, B>, A>) -> Self {
        const { Self::LAYOUT_OK };
        const { Array::<Array<T, B>, A>::LAYOUT_OK };
        // SAFETY: `chunks` is laid out as `Self` (see above).
        unsafe { transmute_layout(chunks) }
    }

    /// Split the [`Array`] into an [`Array`] of chunks.
    pub const fn into_chunks(self) -> Array<Array<T, B>, A> {
        const { Self::LAYOUT_OK };
        const { Array::<Array<T, B>, A>::LAYOUT_OK };
        // SAFETY: `Self` is laid out as `Array<Array<T, B>, A>` (see above).
        unsafe { transmute_layout(self) }
    }

    /// View the [`Array`] as an [`Array`] of chunks.
    pub const fn as_chunks(&self) -> &Array<Array<T, B>, A> {
        // `self` holds `A::USIZE * B::USIZE` elements, so neither call
        // panics.
        let chunks = Array::<T, B>::slice_as_arrays(self.as_slice(), A::USIZE).unwrap();
        Array::from_slice(chunks).unwrap()
    }

    /// View the [`Array`] as a mutable [`Array`] of chunks.
    pub const fn as_chunks_mut(&mut self) -> &mut Array<Array<T, B>, A> {
        // `self` holds `A::USIZE * B::USIZE` elements, so neither call
        // panics.
        let chunks = Array::<T, B>::slice_as_arrays_mut(self.as_mut_slice(), A::USIZE).unwrap();
        Array::from_mut_slice(chunks).unwrap()
    }
}

/// Conversions between plain arrays `[T; N]` and [`Array`]s of size
/// [`Len<N>`]. They are `const`, so they can be used to define constants:
///
/// ```
/// use const_array::{same_len, Array, Len, Sum};
///
/// const PREFIX: Array<u8, Len<4>> = Array::new(*b"conn");
/// const NONCE: Array<u8, Sum<Len<4>, Len<8>>> = PREFIX.concat(Array::new([0; 8]));
/// const FLAT: Array<u8, Len<12>> = NONCE.cast(same_len!(Sum<Len<4>, Len<8>>, Len<12>));
/// assert_eq!(FLAT.as_array()[..4], *b"conn");
/// ```
impl<T, const N: usize> Array<T, Len<N>> {
    /// Wrap a plain array.
    pub const fn new(arr: [T; N]) -> Self {
        Array(arr)
    }

    /// Wrap a reference to a plain array, without copying it.
    pub const fn from_ref(arr: &[T; N]) -> &Self {
        Self::wrap_ref(arr)
    }

    /// Wrap a mutable reference to a plain array, without copying it.
    pub const fn from_mut(arr: &mut [T; N]) -> &mut Self {
        Self::wrap_mut(arr)
    }

    /// View the [`Array`] as a plain array.
    pub const fn as_array(&self) -> &[T; N] {
        &self.0
    }

    /// View the [`Array`] as a mutable plain array.
    pub const fn as_mut_array(&mut self) -> &mut [T; N] {
        &mut self.0
    }

    /// Unwrap the plain array.
    pub const fn into_array(self) -> [T; N] {
        // SAFETY: `Self` is `repr(transparent)` over `[T; N]`. Unlike `self.0`,
        // this is allowed in a `const fn`.
        unsafe { transmute_layout(self) }
    }
}

/// Error when converting a slice into an [`Array`].
#[derive(Debug, Copy, Clone)]
pub struct TryFromSliceError(());

/// Error when an iterator passed to [`Array::try_from_iter`] yields fewer or
/// more elements than the array holds.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TryFromIterError(());

impl fmt::Display for TryFromIterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("iterator length does not match the array length")
    }
}

impl Error for TryFromIterError {}

impl fmt::Display for TryFromSliceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("could not convert slice to array")
    }
}

impl Error for TryFromSliceError {}

impl<'a, T, S: ArrayLen> TryFrom<&'a [T]> for &'a Array<T, S> {
    type Error = TryFromSliceError;

    fn try_from(slice: &'a [T]) -> Result<Self, Self::Error> {
        Array::from_slice(slice).ok_or(TryFromSliceError(()))
    }
}

impl<'a, T, S: ArrayLen> TryFrom<&'a mut [T]> for &'a mut Array<T, S> {
    type Error = TryFromSliceError;

    fn try_from(slice: &'a mut [T]) -> Result<Self, Self::Error> {
        Array::from_mut_slice(slice).ok_or(TryFromSliceError(()))
    }
}

impl<T: Clone, S: ArrayLen> TryFrom<&[T]> for Array<T, S> {
    type Error = TryFromSliceError;

    fn try_from(slice: &[T]) -> Result<Self, Self::Error> {
        <&Self>::try_from(slice).cloned()
    }
}
