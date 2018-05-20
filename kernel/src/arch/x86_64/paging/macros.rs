macro_rules! bit {
    ( $x:expr ) => {
        1 << $x
    };
}

macro_rules! check_flag {
    ($doc:meta, $fun:ident, $flag:ident) => (
        #[$doc]
        pub fn $fun(&self) -> bool {
            self.contains($flag)
        }
    )
}

#[allow(unused_macros)]
macro_rules! is_bit_set {
    ($field:expr, $bit:expr) => (
        $field & (1 << $bit) > 0
    )
}

#[allow(unused_macros)]
macro_rules! check_bit_fn {
    ($doc:meta, $fun:ident, $field:ident, $bit:expr) => (
        #[$doc]
        pub fn $fun(&self) -> bool {
            is_bit_set!(self.$field, $bit)
        }
    )
}
