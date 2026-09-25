use const_array::{at_most, Len, Sum};

fn main() {
    let _proof = at_most!(Sum<Len<16>, Len<8>>, Len<16>);
}
