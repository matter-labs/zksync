//! Generate exit proof for exodus mode given account and token
//! correct verified state should be present in the db (could be restored using `data-restore` module)

use clap::{App, Arg};
use log::info;
use models::node::{AccountId, Address, TokenId, TokenLike};
use models::prover_utils::EncodedProofPlonk;
use num::BigUint;
use serde::Serialize;
use std::time::Instant;
use storage::ConnectionPool;

#[derive(Serialize, Debug)]
struct ExitProofData {
    token_id: TokenId,
    account_id: AccountId,
    account_address: Address,
    amount: BigUint,
    proof: EncodedProofPlonk,
}

fn main() {
    env_logger::init();

    let cli = App::new("Franklin operator node")
        .author("Matter Labs")
        .arg(
            Arg::with_name("Account id")
                .long("accound_id")
                .takes_value(true)
                .required(true)
                .help("Account id of the account"),
        )
        .arg(
            Arg::with_name("Token")
                .long("token")
                .takes_value(true)
                .required(true)
                .help("Token to withdraw - \"ETH\" or address of the ERC20 token"),
        )
        .get_matches();

    let account_id = cli
        .value_of("Account id")
        .expect("required argument")
        .parse::<AccountId>()
        .unwrap();

    let token = {
        let token = cli.value_of("Token").expect("required argument");
        serde_json::from_str::<TokenLike>(token).expect("invalid token argument")
    };

    let timer = Instant::now();
    info!("Restoring state from db");
    let connection_pool = ConnectionPool::new(Some(1));
    let storage = connection_pool
        .access_storage()
        .expect("Storage access failed");

    let token_id = storage
        .tokens_schema()
        .get_token(token)
        .expect("Db access fail")
        .expect("Token not found")
        .id;
    let address = storage
        .chain()
        .account_schema()
        .last_verified_state_for_account(account_id)
        .expect("DB access fail")
        .expect("Account not found in the db")
        .address;
    let accounts = storage
        .chain()
        .state_schema()
        .load_verified_state()
        .expect("Failed to load verified state")
        .1;

    info!("Resotred state from db: {} s", timer.elapsed().as_secs());

    let (proof, amount) =
        prover::exit_proof::create_exit_proof(accounts, account_id, address, token_id)
            .expect("Failed to generate exit proof");

    let proof_data = ExitProofData {
        token_id,
        account_id,
        account_address: address,
        amount,
        proof,
    };

    println!(
        "{}",
        serde_json::to_string(&proof_data).expect("proof data serialize")
    );
}
