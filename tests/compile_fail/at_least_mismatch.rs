use const_array::{at_least, Len, Sum};

fn main() {
    let _proof = at_least!(Len<16>, Sum<Len<16>, Len<8>>);
}
