//! This is Verification key generator for PLONK prover.
//! Verification keys depends on universal setup and circuit,
//! that is why for each version of circuit they can be generated only once.
//! Process of generation of this keys is CPU and memory consuming,
//! so developers are expected to download verification keys from public space.
//! After Verification keys are generated for all of our circuits
//! we can generate verifying contract, that is also deterministic for current circuit version.
//!
//! Only parameters that determine process of these generation is `SUPPORTED_BLOCK_CHUNKS_SIZES`
//! and `SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS` that are read from env in config files.
//! Before generating parameters universal setup keys should be downloaded using `zksync plonk-setup` command.

mod recursive_keys;
mod sample_proofs;
mod verifier_contract_generator;
mod zksync_key;

use structopt::StructOpt;

use crate::recursive_keys::{
    count_gates_recursive_verification_keys, make_recursive_verification_keys,
};
use crate::sample_proofs::make_sample_proofs;
use crate::verifier_contract_generator::create_verifier_contract;
use crate::zksync_key::{
    calculate_and_print_max_zksync_main_circuit_size, make_plonk_blocks_verify_keys,
    make_plonk_exodus_verify_key,
};
use zksync_config::configs::ChainConfig;

#[derive(StructOpt)]
enum Command {
    /// Generate zkSync main circuit(for various block sizes), and exodus circuit verification keys
    Keys,
    /// Generate verifier contract based on verification keys
    Contract,
    /// Counts available sizes (chunks and aggregated proof size) for available setups
    CircuitSize,
}

#[derive(StructOpt)]
#[structopt(name = "ZkSync keys generator", author = "Matter Labs")]
struct Opt {
    #[structopt(subcommand)]
    command: Command,
}

fn main() {
    vlog::init();

    let opt = Opt::from_args();
    let config = ChainConfig::from_env();

    match opt.command {
        Command::Keys => {
            make_plonk_exodus_verify_key();
            make_plonk_blocks_verify_keys(config.clone());
            make_recursive_verification_keys(config.clone());
            make_sample_proofs(config).expect("Failed to generate sample proofs");
        }
        Command::Contract => {
            create_verifier_contract(config);
        }
        Command::CircuitSize => {
            calculate_and_print_max_zksync_main_circuit_size();
            count_gates_recursive_verification_keys();
        }
    }
}
