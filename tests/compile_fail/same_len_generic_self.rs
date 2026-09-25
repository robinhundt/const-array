use const_array::{same_len, ArrayLen, Len, SameLen};

trait Flat {
    type Size: ArrayLen;
    const IS_32: SameLen<Self::Size, Len<32>>;
}

struct Wrapper<S>(S);

// `Self` is generic here, so the macro rejects it during `cargo check`, even
// though every instantiation below would pass.
impl<S: ArrayLen> Flat for Wrapper<S> {
    type Size = S;
    const IS_32: SameLen<S, Len<32>> = same_len!(Self::Size, Len<32>);
}

fn main() {
    let _ = <Wrapper<Len<32>> as Flat>::IS_32;
}
