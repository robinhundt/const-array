use const_array::{at_most, ArrayLen, AtMost, Len};

fn generic<S: ArrayLen>() -> AtMost<S, Len<64>> {
    at_most!(S, Len<64>)
}

fn main() {
    generic::<Len<32>>();
}
