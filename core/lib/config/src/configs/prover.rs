// Built-in uses
use std::time::Duration;
// External uses
use serde::Deserialize;
// Local uses
use crate::envy_load;

/// Configuration for the prover application and part of the server that interact with it.
#[derive(Debug, Deserialize)]
pub struct ProverConfig {
    pub prover: Prover,
    pub core: Core,
    pub witness_generator: WitnessGenerator,
}

impl ProverConfig {
    pub fn from_env() -> Self {
        Self {
            prover: envy_load!("prover.prover", "PROVER_PROVER_"),
            core: envy_load!("prover.core", "PROVER_CORE_"),
            witness_generator: envy_load!("prover.witness_generator", "PROVER_WITNESS_GENERATOR_"),
        }
    }
}

/// Actual prover application settings.
#[derive(Debug, Deserialize)]
pub struct Prover {
    /// Interval of notifying about an ongoing job in ms.
    pub heartbeat_interval: u64,
    /// Interval between the prover rounds in ms.
    pub cycle_wait: u64,
    /// Timeout for the requests to the prover server in seconds.
    pub request_timeout: u64,
}

impl Prover {
    /// Converts `self.heartbeat_interval` into `Duration`.
    pub fn heartbeat_interval(&self) -> Duration {
        Duration::from_millis(self.heartbeat_interval)
    }

    /// Converts `self.cycle_wait` into `Duration`.
    pub fn cycle_wait(&self) -> Duration {
        Duration::from_millis(self.cycle_wait)
    }

    /// Converts `self.request_timeout` into `Duration`.
    pub fn request_timeout(&self) -> Duration {
        Duration::from_secs(self.request_timeout)
    }
}

/// Core settings related to the prover applications interacting with it.
#[derive(Debug, Deserialize)]
pub struct Core {
    /// Timeout to consider prover gone in ms.
    pub gone_timeout: u64,
    /// Amount of provers in the cluser if there is no pending jobs.
    pub idle_provers: u32,
}

impl Core {
    /// Converts `self.gone_timeout` into `Duration`.
    pub fn gone_timeout(&self) -> Duration {
        Duration::from_millis(self.gone_timeout)
    }
}

#[derive(Debug, Deserialize)]
pub struct WitnessGenerator {
    /// Interval to check whether a new witness generation job should be started in ms.
    pub prepare_data_interval: u64,
    /// Amount of witness generator threads.
    pub witness_generators: usize,
}

impl WitnessGenerator {
    /// Converts `self.prepare_data_interval` into `Duration`.
    pub fn prepare_data_interval(&self) -> Duration {
        Duration::from_millis(self.prepare_data_interval)
    }
}
