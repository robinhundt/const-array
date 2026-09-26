# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.6](https://github.com/robinhundt/const-array/compare/v0.1.0-alpha.5...v0.1.0-alpha.6) - 2026-09-26

### Added

- implement Clone for IntoIter, with faster nth, count and last
- implement Borrow and BorrowMut for Array
- add try_from_fn, try_from_iter, each_ref, each_mut and zip
- [**breaking**] add proof lemmas and make proofs invariant
- mark proof constructors and combinators #[must_use]
- print the structure of sizes in Debug

### Other

- call the functions defined in the doctests
- cover the trait impls of arrays, sizes and proofs
- run the no_panic tests in the release profile
- only run the no_panic tests with the no_panic_tests cfg
- count up with a for loop in build
- mark the methods #[inline]
- build arrays flat and in place
- remove the panic paths from try_from_fn and IntoIter drops
- check that infallible checks are optimized away with no-panic
- rewrite the changelog
- document that Array is invariant in T
- [**breaking**] remove ArrayType, Concat and Repeat from the public API
- scrape the examples for the API docs
- add LICENSE, declare MSRV and package metadata

### Added

- Lemmas that prove lengths equal or ordered for all sizes, so generic code
  needs no `checked()` proof, which is only reported by `cargo build`:
  `SameLen::{trans, sum, prod, sum_comm, sum_assoc, sum_zero_left,
  sum_zero_right, prod_comm, prod_assoc, prod_one_left, prod_one_right,
  distrib_left, distrib_right}` and `AtMost::{sum, prod, antisymm,
  prefix_of_sum, suffix_of_sum, zero}`.
- `Array::try_from_fn`, `Array::try_from_iter` (with `TryFromIterError`),
  `Array::each_ref`, `Array::each_mut` and `Array::zip`.
- `Borrow<[T]>` and `BorrowMut<[T]>` for `Array`, e.g. to look up
  `Array` keys in maps by slice.
- `Clone` for `IntoIter`, and faster `nth`, `nth_back`, `count` and `last`.
- `Debug` for `Len`, `Sum` and `Prod` prints the structure, e.g.
  `Sum<Len<12>, Len<4>>`.
- Declared the minimum supported Rust version, 1.85.
- The crate now includes the MIT license text.

### Changed

- **Breaking:** `ArrayType`, `Concat` and `Repeat` are no longer part of the
  public API. They moved to the hidden `__private` module, and the fields of
  `Concat` and `Repeat` are private.
- **Breaking:** `SameLen` and `AtMost` are invariant in their size
  parameters. All sizes are `'static`, so this should not affect any code.
- **Breaking:** The `Debug` impls of `Sum` and `Prod` require their parameters
  to implement `Debug` and `Default`, which every `ArrayLen` does.
- The proofs' constructors and combinators are `#[must_use]`.

## [0.1.0-alpha.5](https://github.com/robinhundt/const-array/compare/v0.1.0-alpha.4...v0.1.0-alpha.5) - 2026-09-25

### Changed

- Less unsafe code: reference casts and splits go through checked slice
  conversions.
- Improved the documentation of the examples.

## [0.1.0-alpha.4](https://github.com/robinhundt/const-array/compare/v0.1.0-alpha.3...v0.1.0-alpha.4) - 2026-09-25

### Added

- `Array::LEN`, the length as a constant.
- `Array::slice_as_chunks` and `Array::slice_as_chunks_mut`.
- `Array::suffix_ref`, `suffix_mut`, `split_suffix` and `split_suffix_mut`.
- The `AtLeast` alias and the `at_least!` macro.

### Changed

- `same_len!` and `at_most!` accept `Self` and its associated types in impls
  for concrete types, and `at_most!` errors name both lengths.
- Releases are published by release-plz.

## [0.1.0-alpha.3] - 2026-09-25

### Changed

- **Breaking:** `Array::concat(a, b)` is now the method `a.concat(b)`.
- **Breaking:** `Len` is a unit struct, and every `ArrayLen` implements
  `Copy`, `Debug`, `Default`, `Eq`, `Ord` and `Hash`, and is
  `Send + Sync + 'static`, so derives on types generic over a size work.

### Added

- `PartialEq` between `Array<T, Len<N>>` and `[T; N]`.
- `Display` and `Error` for `TryFromSliceError`.

## [0.1.0-alpha.2] - 2026-09-25

### Added

- `Prod` sizes, `SameLen` and `AtMost` proofs with the `same_len!` and
  `at_most!` macros, and the cryptographic API examples.

## [0.1.0-alpha.1] - 2026-09-25

- Initial release.
