use const_array::{same_len, Sum, Len};

fn main() {
    let _proof = same_len!(Len<16>, Sum<Len<16>, Len<8>>);
}
