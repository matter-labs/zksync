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
    use zksync_config::configs::eth_sender::ETHSenderConfig;

    /// Obtains the interval for renewing the maximum gas price.
    ///
    /// This value is not cached internally, as it may be changed for the already running
    /// server by an administrator. This may be required if existing settings aren't flexible
    /// enough to match the current network price.
    pub fn limit_update_interval() -> Duration {
        let config = ETHSenderConfig::from_env();
        config.gas_price_limit.update_interval()
    }

    /// Obtains the scaling factor for the maximum gas price.
    ///
    /// This value is not cached internally, as it may be changed for the already running
    /// server by an administrator. This may be required if existing settings aren't flexible
    /// enough to match the current network price.
    pub fn limit_scale_factor() -> f64 {
        let config = ETHSenderConfig::from_env();
        config.gas_price_limit.scale_factor
    }

    /// Obtains the interval for the gas price samples to be added into `gas_adjuster`.
    ///
    /// This value is not cached internally, as it may be changed for the already running
    /// server by an administrator. This may be required if existing settings aren't flexible
    /// enough to match the current network price.
    pub fn sample_adding_interval() -> Duration {
        let config = ETHSenderConfig::from_env();
        config.gas_price_limit.sample_interval()
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
