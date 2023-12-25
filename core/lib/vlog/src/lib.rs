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

use chrono::Duration;
use std::{borrow::Cow, str::FromStr};

pub use sentry;
use sentry::protocol::Event;
use sentry::{types::Dsn, ClientInitGuard, ClientOptions};

pub use tracing as __tracing;
pub use tracing::{debug, info, log, trace};
use tracing_appender::non_blocking::WorkerGuard;

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

/// When this is dropped sentry and logger stops working
pub struct VlogGuard {
    _sentry_guard: Option<ClientInitGuard>,
    _logger_guard: WorkerGuard,
}

fn get_sentry_url() -> Option<Dsn> {
    if let Ok(sentry_url) = std::env::var("MISC_SENTRY_URL") {
        if let Ok(sentry_url) = Dsn::from_str(sentry_url.as_str()) {
            return Some(sentry_url);
        }
    }
    None
}

/// Initialize logging with non blocking tracing and set up log format
///
/// If the sentry URL is provided via an environment variable, this function will also initialize sentry.
/// Returns a VlogGuard guard. Which contains Sentry Guard and Logger Guard
///
/// The full description can be found in the official documentation:
/// https://docs.sentry.io/platforms/rust/#configure
/// https://docs.rs/tracing-appender/0.2.2/tracing_appender/non_blocking/index.html
pub fn init() -> VlogGuard {
    let log_format = std::env::var("MISC_LOG_FORMAT").unwrap_or_else(|_| "plain".to_string());
    let (non_blocking, _logger_guard) = tracing_appender::non_blocking(std::io::stdout());
    match log_format.as_str() {
        "plain" => {
            tracing_subscriber::fmt::Subscriber::builder()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .with_writer(non_blocking)
                .init();
        }
        "json" => {
            let timer = tracing_subscriber::fmt::time::ChronoUtc::rfc3339();
            tracing_subscriber::fmt::Subscriber::builder()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .with_writer(non_blocking)
                .with_timer(timer)
                .json()
                .init();
        }
        _ => panic!("MISC_LOG_FORMAT has an unexpected value {}", log_format),
    };

    let _sentry_guard = get_sentry_url().map(|sentry_url| {
        let options = sentry::ClientOptions {
            release: sentry::release_name!(),
            environment: Some(Cow::from(
                std::env::var("CHAIN_ETH_NETWORK").expect("Must be set"),
            )),
            attach_stacktrace: true,
            ..Default::default()
        };
        let options = options.add_integration(AddIntervalToFingerprintIntegration::new(
            Duration::seconds(10),
            Duration::minutes(10),
        ));
        sentry::init((sentry_url, options))
    });
    VlogGuard {
        _sentry_guard,
        _logger_guard,
    }
}

struct AddIntervalToFingerprintIntegration {
    panic_interval: Duration,
    error_interval: Duration,
}

impl AddIntervalToFingerprintIntegration {
    fn new(panic_interval: Duration, error_interval: Duration) -> Self {
        Self {
            panic_interval,
            error_interval,
        }
    }
}

impl sentry::Integration for AddIntervalToFingerprintIntegration {
    fn process_event(
        &self,
        mut event: Event<'static>,
        _options: &ClientOptions,
    ) -> Option<Event<'static>> {
        let mut fingerprints = match event.fingerprint {
            Cow::Borrowed(slice) => slice.to_vec(),
            Cow::Owned(vec) => vec,
        };
        if event.level == sentry::Level::Fatal {
            let message = event
                .exception
                .first()
                .and_then(|exception| exception.value.as_ref().cloned())
                .unwrap_or_default();
            fingerprints.push(Cow::Owned(message));
        }
        let interval = match event.level {
            sentry::Level::Fatal => self.panic_interval.num_seconds(),
            _ => self.error_interval.num_seconds(),
        };
        let time_fingerprint = chrono::Utc::now().timestamp() / interval;
        fingerprints.push(Cow::Owned(time_fingerprint.to_string()));
        event.fingerprint = Cow::Owned(fingerprints);
        Some(event)
    }
}
