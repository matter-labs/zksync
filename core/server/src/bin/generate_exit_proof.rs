//! Generate exit proof for exodus mode given account and token
//! correct verified state should be present in the db (could be restored using `data-restore` module)

use bigdecimal::BigDecimal;
use clap::{App, Arg};
use log::info;
use models::node::{Address, TokenId, TokenLike};
use models::prover_utils::EncodedProofPlonk;
use serde::Serialize;
use std::time::Instant;
use storage::ConnectionPool;

#[derive(Serialize, Debug)]
struct ExitProofData {
    token_id: TokenId,
    owner: Address,
    amount: BigDecimal,
    proof: EncodedProofPlonk,
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

    let target_token = {
        let token = cli.value_of("Token").expect("required argument");
        serde_json::from_str::<TokenLike>(token).expect("invalid token argument")
    };

    let timer = Instant::now();
    info!("Restoring state from db");
    let connection_pool = ConnectionPool::new();
    let storage = connection_pool
        .access_storage()
        .expect("Storage access failed");

    let token_id = storage
        .tokens_schema()
        .get_token(target_token)
        .expect("Db access fail")
        .expect("Token not found")
        .id;
    let accounts = storage
        .chain()
        .state_schema()
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
