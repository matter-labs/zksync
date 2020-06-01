#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        log::warn!(
            "[{}:{}:{}] {}",
            file!(),
            line!(),
            column!(),
            format!($($arg)*)
        );
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        log::error!(
            "[{}:{}:{}] {}",
            file!(),
            line!(),
            column!(),
            format!($($arg)*)
        );
    };
}
