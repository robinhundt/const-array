# Const Array

This library is an exploration of the design space for const generic arrays,
similar to [generic-array][ga] or or [hybrid-array][ha]. The main difference
is that it does not use [typenum][tn] at all and relies solely on
const generics and some unsafe code.

Currently, this is very limited in it's functionality. Some potentially
useful operations would require adding post-monomorphization errors.

[ga]: https://crates.io/crates/generic-array
[ha]: https://crates.io/crates/hybrid-array
[tn]: https://crates.io/crates/typenum