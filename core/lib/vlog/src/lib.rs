//! A set of logging macros that print not only timestamp and log level,
//! but also filename, line and column.
//!
//! They behave just like usual log::warn, tracing::info, etc.
//!
//!
//! In fact, printing file, line and column can be done with a custom formatter for env_logger, like so:
//!
//!```ignore
//! use env_logger::Builder;
//! use std::io::Write;
//!
//! env_logger::builder()
//!     .format(|buf, record| {
//!         writeln!(buf, "{:?}", record.file());
//!         writeln!(buf, "{}", record.args())
//!     })
//!     .init();
//!```
//!
//! But I couldn't easily replicate its default behavior in my custom logger.
//!
pub use tracing::{debug, info, trace};

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        tracing::warn!(
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
        tracing::error!(
            "[{}:{}:{}] {}",
            file!(),
            line!(),
            column!(),
            format!($($arg)*)
        );
    };
}
