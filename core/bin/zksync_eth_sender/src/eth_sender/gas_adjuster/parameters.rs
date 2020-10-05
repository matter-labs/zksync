//! `parameters` module provides methods to get configurable parameters of `GasAdjuster`.
//!
//! Currently the following types of parameters are provided:
//! - Maximum gas price renewal interval: interval between updates of the upper limit for
//!   gas price suggested by `GasAdjuster`.
//! - Maximum gas price scale: multiplier to be applied to the average gas price to
//!   calculate the upper limit for gas price in `GasAdjuster`.
//!
//! The module uses a child module `parameters_impl` which contains two implementations
//! for functions declared in module: one for the actual usage, and one for tests.
//! While the actual implementation obtains the values from the environment variables,
//! the test one uses hard-coded values for better test behavior predictability.

// Built-in deps.
use std::time::Duration;

/// Obtains the interval for renewing the maximum gas price.
///
/// This value is not cached internally, as it may be changed for the already running
/// server by an administrator. This may be required if existing settings aren't flexible
/// enough to match the current network price.
pub fn limit_update_interval() -> Duration {
    parameters_impl::limit_update_interval()
}

/// Obtains the scaling factor for the maximum gas price.
///
/// This value is not cached internally, as it may be changed for the already running
/// server by an administrator. This may be required if existing settings aren't flexible
/// enough to match the current network price.
pub fn limit_scale_factor() -> f64 {
    parameters_impl::limit_scale_factor()
}

/// Obtains the interval for the gas price samples to be added into `gas_adjuster`.
///
/// This value is not cached internally, as it may be changed for the already running
/// server by an administrator. This may be required if existing settings aren't flexible
/// enough to match the current network price.
pub fn sample_adding_interval() -> Duration {
    parameters_impl::sample_adding_interval()
}

// Actual methods implementation for non-test purposes.
#[cfg(not(test))]
mod parameters_impl {
    // Built-in deps.
    use std::time::Duration;
    // Workspace deps
    use zksync_utils::parse_env;

    /// Name of the environment variable responsible for the `gas_price_limit` renewing interval.
    const GAS_PRICE_LIMIT_UPDATE_INTERVAL: &str = "ETH_GAS_PRICE_LIMIT_UPDATE_INTERVAL";
    /// Name of the environment variable responsible for the `gas_price_limit` scaling multiplier.
    const GAS_PRICE_LIMIT_SCALE_FACTOR: &str = "ETH_GAS_PRICE_LIMIT_SCALE_FACTOR";
    /// Name of the environment variable responsible for the interval between adding gas prices to the `gas_adjuster`.
    const GAS_PRICE_LIMIT_SAMPLE_INTERVAL: &str = "ETH_GAS_PRICE_LIMIT_SAMPLE_INTERVAL";

    /// Interval between adding the Ethereum node gas price to the GasAdjuster (in seconds).
    /// This value will be used if no `ETH_GAS_PRICE_LIMIT_SAMPLE_INTERVAL` is set in env.
    /// Defaults to 15 seconds (1 Ethereum block)
    const DEFAULT_GAS_PRICE_LIMIT_SAMPLE_INTERVAL: Duration = Duration::from_secs(15);

    /// Obtains the interval for renewing the maximum gas price.
    ///
    /// This value is not cached internally, as it may be changed for the already running
    /// server by an administrator. This may be required if existing settings aren't flexible
    /// enough to match the current network price.
    pub fn limit_update_interval() -> Duration {
        let renew_interval: u64 = parse_env(GAS_PRICE_LIMIT_UPDATE_INTERVAL);

        Duration::from_secs(renew_interval)
    }

    /// Obtains the scaling factor for the maximum gas price.
    ///
    /// This value is not cached internally, as it may be changed for the already running
    /// server by an administrator. This may be required if existing settings aren't flexible
    /// enough to match the current network price.
    pub fn limit_scale_factor() -> f64 {
        parse_env(GAS_PRICE_LIMIT_SCALE_FACTOR)
    }

    /// Obtains the interval for the gas price samples to be added into `gas_adjuster`.
    ///
    /// This value is not cached internally, as it may be changed for the already running
    /// server by an administrator. This may be required if existing settings aren't flexible
    /// enough to match the current network price.
    pub fn sample_adding_interval() -> Duration {
        if std::env::var(GAS_PRICE_LIMIT_SAMPLE_INTERVAL).is_err() {
            log::trace!(
                "No value provided for `ETH_GAS_PRICE_LIMIT_SAMPLE_INTERVAL` env variable, \
                 using the default: {} seconds",
                DEFAULT_GAS_PRICE_LIMIT_SAMPLE_INTERVAL.as_secs()
            );
            return DEFAULT_GAS_PRICE_LIMIT_SAMPLE_INTERVAL;
        }

        let renew_interval: u64 = parse_env(GAS_PRICE_LIMIT_SAMPLE_INTERVAL);

        Duration::from_secs(renew_interval)
    }
}

// Hard-coded implementation for tests.
#[cfg(test)]
mod parameters_impl {
    // Built-in deps.
    use std::time::Duration;

    /// `limit_update_interval` version for tests not looking for an environment variable value
    /// but using a zero interval instead.
    pub fn limit_update_interval() -> Duration {
        Duration::from_secs(0)
    }

    /// `limit_scale_factor` version for tests not looking for an environment variable value
    /// but using a fixed scale factor (1.5) instead.
    pub fn limit_scale_factor() -> f64 {
        1.5f64
    }

    /// `sample_adding_interval` version for tests not looking for an environment variable value
    /// but using a zero interval instead.
    pub fn sample_adding_interval() -> Duration {
        Duration::from_secs(0)
    }
}
