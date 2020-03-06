#[macro_use]
extern crate log;

pub mod franklin_key;
pub mod vk_contract_generator;

use crate::franklin_key::{make_block_proof_key, make_exodus_key};
use crate::vk_contract_generator::compose_verifer_keys_contract;
use clap::{App, SubCommand};

fn main() {
    env_logger::init();

    let cli = App::new("Zksync keys generator")
        .author("Matter Labs")
        .subcommand(SubCommand::with_name("block").about("Generate block proof"))
        .subcommand(SubCommand::with_name("exodus_key").about("Generate exodus exit key"))
        .subcommand(SubCommand::with_name("contract").about("Generate verify contract"))
        .get_matches();

    let (cmd, _) = cli.subcommand();
    if cmd == "block" {
        make_block_proof_key();
    } else if cmd == "exodus_key" {
        make_exodus_key();
    } else if cmd == "contract" {
        compose_verifer_keys_contract();
    }
}
