use const_array::{same_len, ArrayLen, Len, SameLen, Sum};

trait Params {
    type K: ArrayLen;
    type KeySize: ArrayLen;
    const KEY_PARTS: SameLen<Self::KeySize, Sum<Self::K, Len<32>>>;
}

struct P;

// Reported by `cargo check`, although nothing uses `KEY_PARTS`.
impl Params for P {
    type K = Len<33>;
    type KeySize = Len<64>;
    const KEY_PARTS: SameLen<Self::KeySize, Sum<Self::K, Len<32>>> =
        same_len!(Self::KeySize, Sum<Self::K, Len<32>>);
}

fn main() {}
