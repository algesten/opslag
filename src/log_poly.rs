#![allow(unused_macro_rules)]

macro_rules! trace {
    (target: $target:expr, $($arg:tt)+) => {};
    ($($arg:tt)+) => {};
}

macro_rules! debug {
    (target: $target:expr, $($arg:tt)+) => {};
    ($($arg:tt)+) => {};
}

macro_rules! warn {
    (target: $target:expr, $($arg:tt)+) => {};
    ($($arg:tt)+) => {};
}
