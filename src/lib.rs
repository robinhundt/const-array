#![doc = include_str!("../README.md")]
#![no_std]
#![deny(clippy::undocumented_unsafe_blocks)]

mod array;
mod at_most;
mod impls;
mod iter;
mod same_len;
mod size;

pub use array::{Array, TryFromIterError, TryFromSliceError};
pub use at_most::{AtLeast, AtMost};
pub use iter::IntoIter;
pub use same_len::SameLen;
pub use size::{ArrayLen, Len, Prod, Sum};

/// Implementation details that must be public because they appear in
/// [`ArrayLen::ArrayType`]. They are not part of the public API and may change
/// in any release.
#[doc(hidden)]
pub mod __private {
    pub use crate::size::{ArrayType, Concat, Repeat};
}

mod sealed {
    use crate::{
        Len, Prod, Sum,
        size::{Concat, Repeat},
    };

    pub trait Sealed {}

    impl<T, const N: usize> Sealed for [T; N] {}
    impl<A, B> Sealed for Concat<A, B> {}
    impl<const N: usize> Sealed for Len<N> {}
    impl<A, B> Sealed for Sum<A, B> {}
    impl<I, O> Sealed for Repeat<I, O> {}
    impl<A, B> Sealed for Prod<A, B> {}
}
