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

mod verifier_contract_generator;
mod zksync_key;

use structopt::StructOpt;

use crate::verifier_contract_generator::create_verifier_contract;
use crate::zksync_key::{make_plonk_blocks_verify_keys, make_plonk_exodus_verify_key};
use zksync_config::AvailableBlockSizesConfig;

#[derive(StructOpt)]
enum Command {
    /// Generate zkSync main circuit(for various block sizes), and exodus circuit verification keys
    Keys,
    /// Generate verifier contract based on verification keys
    Contract,
}

#[derive(StructOpt)]
#[structopt(name = "ZkSync keys generator", author = "Matter Labs")]
struct Opt {
    #[structopt(subcommand)]
    command: Command,
}

fn main() {
    env_logger::init();

    let opt = Opt::from_args();
    let config = AvailableBlockSizesConfig::from_env();

    match opt.command {
        Command::Keys => {
            make_plonk_exodus_verify_key();
            make_plonk_blocks_verify_keys(config);
        }
        Command::Contract => {
            create_verifier_contract(config);
        }
    }
}
