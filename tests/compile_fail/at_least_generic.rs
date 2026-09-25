use const_array::{at_least, ArrayLen, AtLeast, Len};

fn generic<S: ArrayLen>() -> AtLeast<S, Len<1>> {
    at_least!(S, Len<1>)
}

fn main() {
    let _ = generic::<Len<4>>();
}
