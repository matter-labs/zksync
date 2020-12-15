// External uses
use serde::Deserialize;
// Local uses
use crate::{envy_load, toml_load};

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

    pub fn from_toml(path: &str) -> Self {
        toml_load!("eth_sender", path)
    }
}

/// Actual prover application settings.
#[derive(Debug, Deserialize)]
pub struct Prover {
    pub heartbeat_interval: u64,
    pub cycle_wait: u64,
    pub request_timeout: u64,
}

/// Core settings related to the prover applications interacting with it.
#[derive(Debug, Deserialize)]
pub struct Core {
    pub gone_timeout: u64,
    pub idle_provers: u32,
}

#[derive(Debug, Deserialize)]
pub struct WitnessGenerator {
    pub prepare_data_interval: u64,
    pub witness_generators: usize,
}
