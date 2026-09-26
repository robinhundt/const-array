// `Array` is invariant in its element type, because its storage is an
// associated type. So unlike `[&'static str; 2]`, an array of `&'static str`
// can't be passed where an array of shorter-lived references is expected.
// If this ever compiles, update the "Variance" section of the README.
use const_array::{Array, Len};

fn shorten<'a>(a: Array<&'static str, Len<2>>) -> Array<&'a str, Len<2>> {
    a
}

fn main() {
    let _ = shorten(Array::new(["a", "b"]));
}
