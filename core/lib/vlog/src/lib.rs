//! A set of logging macros that print not only timestamp and log level,
//! but also filename, line and column.
//!
//! They behave just like usual log::warn, vlog::info, etc.
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
pub use tracing as __tracing;
pub use tracing::{debug, info, trace};

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        vlog::__tracing::warn!(
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
        vlog::__tracing::error!(
            "[{}:{}:{}] {}",
            file!(),
            line!(),
            column!(),
            format!($($arg)*)
        );
    };
}

pub fn init() {
    let log_format = std::env::var("MISC_LOG_FORMAT").unwrap_or("plain".to_string());
    match log_format.as_str() {
        "plain" => tracing_subscriber::fmt::init(),
        "json" => {
            tracing_subscriber::fmt::Subscriber::builder()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .json()
                .init();
        }
        _ => panic!("MISC_LOG_FORMAT has an unexpected value"),
    };
}
