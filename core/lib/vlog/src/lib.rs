//! A set of logging macros that print not only timestamp and log level,
//! but also filename, line and column.
//!
//! They behave just like usual tracing::warn, tracing::info, etc.
//! For warn and error macros we are adding file line and column to tracing variables
//!
//! The format of the logs in stdout can be `plain` or` json` and is set by the `MISC_LOG_FORMAT` env variable.
//!
//! Full documentation for the `tracing` crate here https://docs.rs/tracing/
//!
//! Integration with sentry for catching errors and react on them immediately
//! https://docs.sentry.io/platforms/rust/
//!

use std::{borrow::Cow, str::FromStr};

pub use sentry;
use sentry::{types::Dsn, ClientInitGuard};

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

fn get_sentry_url() -> Option<Dsn> {
    if let Ok(sentry_url) = std::env::var("MISC_SENTRY_URL") {
        if let Ok(sentry_url) = Dsn::from_str(sentry_url.as_str()) {
            return Some(sentry_url);
        }
    }
    None
}

/// Initialize logging with tracing and set up log format
///
/// If the sentry URL is provided via an environment variable, this function will also initialize sentry.
/// Returns a sentry client guard. The full description can be found in the official documentation:
/// https://docs.sentry.io/platforms/rust/#configure
pub fn init() -> Option<ClientInitGuard> {
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

    get_sentry_url().map(|sentry_url| {
        sentry::init((
            sentry_url,
            sentry::ClientOptions {
                release: sentry::release_name!(),
                environment: Some(Cow::from(
                    std::env::var("CHAIN_ETH_NETWORK").expect("Must be set"),
                )),
                attach_stacktrace: true,
                ..Default::default()
            },
        ))
    })
}

#[cfg(feature = "actix")]
pub fn actix_middleware() -> sentry_actix::Sentry {
    sentry_actix::Sentry::new()
}
