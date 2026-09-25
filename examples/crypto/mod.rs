//! Shared code for the examples.
//!
//! - [`traits`]: how common cryptographic traits look when their sizes are
//!   [`ArrayLen`](const_array::ArrayLen)s and their fixed-size inputs and
//!   outputs are [`Array`](const_array::Array)s.
//! - [`hmac`]: a generic construction over any [`traits::Digest`].
//! - [`fake`]: **insecure** stand-ins for the primitives, so the examples can
//!   run. They only have the right shapes, not the right behavior.

pub mod fake;
pub mod hmac;
pub mod traits;
