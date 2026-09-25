use const_array::{same_len, Array, Len};

fn main() {
    let a: Array<u8, Len<3>> = Array::default();
    let _b = a.cast(same_len!(Len<2>, Len<2>));
}
