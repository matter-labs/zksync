//! Generate exit proof for exodus mode given account and token
//! correct verified state should be present in the db (could be restored using `data-restore` module)

use bigdecimal::BigDecimal;
use clap::{App, Arg};
use log::info;
use models::node::{Address, TokenId};
use models::EncodedProof;
use serde::Serialize;
use std::time::Instant;
use storage::{
    interfaces::{state::StateSchema, tokens::TokensSchema},
    ConnectionPool,
};

#[derive(Serialize, Debug)]
struct ExitProofData {
    token_id: TokenId,
    owner: Address,
    amount: BigDecimal,
    proof: EncodedProof,
}

fn main() {
    env_logger::init();

    let cli = App::new("Franklin operator node")
        .author("Matter Labs")
        .arg(
            Arg::with_name("Address")
                .long("address")
                .takes_value(true)
                .required(true)
                .help("Account address of the account"),
        )
        .arg(
            Arg::with_name("Token")
                .long("token")
                .takes_value(true)
                .required(true)
                .help("Token to withdraw - \"ETH\" or address of the ERC20 token"),
        )
        .get_matches();

    let target_account_address: Address = {
        let address = cli.value_of("Address").expect("required argument");
        let value_to_parse = if address.starts_with("0x") {
            &address[2..]
        } else {
            address
        };
        value_to_parse
            .parse()
            .expect("Address should be valid account address")
    };

    let target_token_address = {
        let token = cli.value_of("Token").expect("required argument");
        if token == "ETH" {
            token.to_string()
        } else {
            let token_address_to_parse = if token.starts_with("0x") {
                &token[2..]
            } else {
                token
            };
            let address: Address = token_address_to_parse
                .parse()
                .expect("Token address should be valid ERC20 address");
            format!("0x{:x}", address)
        }
    };

    let timer = Instant::now();
    info!("Restoring state from db");
    let connection_pool = ConnectionPool::new();
    let storage = connection_pool
        .access_storage()
        .expect("Storage access failed");

    let token_id = if target_token_address == "ETH" {
        0
    } else {
        let tokens = TokensSchema(&storage)
            .load_tokens()
            .expect("Failed to load token");
        tokens
            .into_iter()
            .find(|(_, token)| token.address == target_token_address)
            .expect("Token not found")
            .0
    };
    let accounts = StateSchema(&storage)
        .load_verified_state()
        .expect("Failed to load verified state")
        .1;

    info!("Resotred state from db: {} s", timer.elapsed().as_secs());

    let (proof, amount) =
        prover::exit_proof::create_exit_proof(accounts, target_account_address, token_id)
            .expect("Failed to generate exit proof");

    let proof_data = ExitProofData {
        token_id,
        owner: target_account_address,
        amount,
        proof,
    };

    println!(
        "{}",
        serde_json::to_string(&proof_data).expect("proof data serialize")
    );
}
