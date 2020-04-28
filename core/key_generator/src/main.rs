//! This is Verification key generator for PLONK prover.
//! Verification keys depends on universal setup and circuit,
//! jthat is why for each version of circuit they can be generated only once.
//! Process of generation of this keys is CPU and memory consuming,
//! so developers are expected to download verification keys from public space.
//! After Verification keys are generated for all of our circuits
//! we can generate verifying contract, that is also deterministic for current circuit version.
//!
//! Only parameters that determine process of these generation is `SUPPORTED_BLOCK_CHUNKS_SIZES`
//! and `SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS` that are read from env in config files.
//! Before generating parameters universal setup keys should be downloaded using `zksync plonk-setup` command.

mod franklin_key;
mod verifier_contract_generator;

use clap::{App, SubCommand};

use crate::franklin_key::{make_plonk_blocks_verify_keys, make_plonk_exodus_verify_key};
use crate::verifier_contract_generator::create_verifier_contract;
use models::config_options::AvailableBlockSizesConfig;

fn main() {
    env_logger::init();

    let cli = App::new("Zksync keys generator")
        .author("Matter Labs")
        .subcommand(
            SubCommand::with_name("keys").about("Generate zkSync main circuit(for various block sizes), and exodus circuit verification keys"),
        )
        .subcommand(SubCommand::with_name("contract").about("Generate verifier contract based on verification keys"))
        .get_matches();

    let config = AvailableBlockSizesConfig::from_env();
    let (cmd, _) = cli.subcommand();
    if cmd == "keys" {
        make_plonk_exodus_verify_key();
        make_plonk_blocks_verify_keys(config);
    } else if cmd == "contract" {
        create_verifier_contract(config);
    }
}
