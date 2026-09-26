# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0-alpha.6](https://github.com/robinhundt/const-array/compare/v0.1.0-alpha.5...v0.1.0-alpha.6) - 2026-09-26

### Added

- Lemmas that prove lengths equal or ordered for all sizes, so generic code
  needs no `checked()` proof, which is only reported by `cargo build`:
  `SameLen::{trans, sum, prod, sum_comm, sum_assoc, sum_zero_left,
  sum_zero_right, prod_comm, prod_assoc, prod_one_left, prod_one_right,
  distrib_left, distrib_right}` and `AtMost::{sum, prod, antisymm,
  prefix_of_sum, suffix_of_sum, zero}`
  ([#19](https://github.com/robinhundt/const-array/pull/19)).
- `Array::try_from_fn`, `Array::try_from_iter` (with `TryFromIterError`),
  `Array::each_ref`, `Array::each_mut` and `Array::zip`
  ([#19](https://github.com/robinhundt/const-array/pull/19)).
- `Borrow<[T]>` and `BorrowMut<[T]>` for `Array`, e.g. to look up
  `Array` keys in maps by slice
  ([#19](https://github.com/robinhundt/const-array/pull/19)).
- `Clone` for `IntoIter`, and faster `nth`, `nth_back`, `count` and `last`
  ([#19](https://github.com/robinhundt/const-array/pull/19)).
- `Debug` for `Len`, `Sum` and `Prod` prints the structure, e.g.
  `Sum<Len<12>, Len<4>>`
  ([#19](https://github.com/robinhundt/const-array/pull/19)).
- Declared the minimum supported Rust version, 1.85
  ([#19](https://github.com/robinhundt/const-array/pull/19)).
- The crate now includes the MIT license text
  ([#19](https://github.com/robinhundt/const-array/pull/19)).

### Changed

- **Breaking:** `ArrayType`, `Concat` and `Repeat` are no longer part of the
  public API. They moved to the hidden `__private` module, and the fields of
  `Concat` and `Repeat` are private
  ([#19](https://github.com/robinhundt/const-array/pull/19)).
- **Breaking:** `SameLen` and `AtMost` are invariant in their size
  parameters. All sizes are `'static`, so this should not affect any code
  ([#19](https://github.com/robinhundt/const-array/pull/19)).
- **Breaking:** The `Debug` impls of `Sum` and `Prod` require their parameters
  to implement `Debug` and `Default`, which every `ArrayLen` does
  ([#19](https://github.com/robinhundt/const-array/pull/19)).
- The proofs' constructors and combinators are `#[must_use]`
  ([#19](https://github.com/robinhundt/const-array/pull/19)).
- Arrays of `Sum` and `Prod` sizes are built flat and in place, so `from_fn`
  and `clone` compile like for a flat `Len`, e.g. `clone` to a single
  `memcpy` ([#24](https://github.com/robinhundt/const-array/pull/24)).
- All methods are `#[inline]`, and the panic paths in `try_from_fn` and the
  drop of `IntoIter` are removed, so checks that can't fail are optimized away
  ([#24](https://github.com/robinhundt/const-array/pull/24)).

## [0.1.0-alpha.5](https://github.com/robinhundt/const-array/compare/v0.1.0-alpha.4...v0.1.0-alpha.5) - 2026-09-25

### Changed

- Less unsafe code: reference casts and splits go through checked slice
  conversions ([#18](https://github.com/robinhundt/const-array/pull/18)).
- Improved the documentation of the examples
  ([#17](https://github.com/robinhundt/const-array/pull/17)).

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
