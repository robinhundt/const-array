#![doc = include_str!("../README.md")]
#![no_std]
#![deny(clippy::undocumented_unsafe_blocks)]

mod array;
mod at_most;
mod impls;
mod iter;
mod same_len;
mod size;

pub use array::{Array, TryFromSliceError};
pub use at_most::{AtLeast, AtMost};
pub use iter::IntoIter;
pub use same_len::SameLen;
pub use size::{ArrayLen, ArrayType, Concat, Len, Prod, Repeat, Sum};

mod sealed {
    use crate::{Concat, Len, Prod, Repeat, Sum};

    pub trait Sealed {}

    impl<T, const N: usize> Sealed for [T; N] {}
    impl<A, B> Sealed for Concat<A, B> {}
    impl<const N: usize> Sealed for Len<N> {}
    impl<A, B> Sealed for Sum<A, B> {}
    impl<I, O> Sealed for Repeat<I, O> {}
    impl<A, B> Sealed for Prod<A, B> {}
}
