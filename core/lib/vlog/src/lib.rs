//! A set of logging macros that print not only timestamp and log level,
//! but also filename, line and column.
//!
//! They behave just like usual tracing::warn, tracing::info, etc.
//! For warn and error macros we are adding file line and column to tracing variables
//!
//! The format of the logs in stdout can be `plain` or` json` and is set by the `MISC_LOG_FORMAT` env variable.
//!
//! Full documentation for the `tracing` crate here https://docs.rs/tracing/

pub use tracing as __tracing;
pub use tracing::{debug, info, log, trace};

#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => {
        vlog::__tracing::warn!(
            file=file!(),
            line=line!(),
            column=column!(),
            $($arg)*
        );
    };
}

#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => {
        vlog::__tracing::error!(
            file=file!(),
            line=line!(),
            column=column!(),
            $($arg)*
        );
    };
}

pub fn init() {
    let log_format = std::env::var("MISC_LOG_FORMAT").unwrap_or_else(|_| "plain".to_string());
    match log_format.as_str() {
        "plain" => tracing_subscriber::fmt::init(),
        "json" => {
            let timer = tracing_subscriber::fmt::time::ChronoUtc::rfc3339();
            tracing_subscriber::fmt::Subscriber::builder()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .with_timer(timer)
                .json()
                .init();
        }
        _ => panic!("MISC_LOG_FORMAT has an unexpected value {}", log_format),
    };
}
