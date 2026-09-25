use const_array::{same_len, ArrayLen, Len};

fn generic<S: ArrayLen>() {
    let _proof = same_len!(S, Len<32>);
}

fn main() {
    generic::<Len<32>>();
}
