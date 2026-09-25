use const_array::{Array, Len};

fn main() {
    let mut a: Array<u8, Len<16>> = Array::default();
    let b: Array<u8, Len<32>> = Array::default();
    a.zip_mut_with(&b, |x, y| *x ^= y);
}
