//! Generate exit proof for exodus mode given account and token
//! correct verified state should be present in the db (could be restored using `data-restore` module)

use bigdecimal::BigDecimal;
use circuit::exit_circuit::create_exit_circuit_with_public_input;
use clap::{App, Arg};
use franklin_crypto::bellman::groth16::Parameters;
use log::info;
use models::circuit::account::CircuitAccount;
use models::circuit::CircuitAccountTree;
use models::node::{Address, Engine, TokenId};
use models::prover_utils::{
    create_random_full_baby_proof, encode_proof, read_circuit_proving_parameters,
    verify_full_baby_proof,
};
use models::EncodedProof;
use serde::Serialize;
use std::time::Instant;
use storage::ConnectionPool;

fn read_parameters() -> Parameters<Engine> {
    let timer = Instant::now();
    let path = {
        let mut key_file_path = std::path::PathBuf::new();
        key_file_path.push(&std::env::var("KEY_DIR").expect("KEY_DIR is not set"));
        key_file_path.push(&format!("{}", models::params::block_size_chunks()));
        key_file_path.push(&format!("{}", models::params::account_tree_depth()));
        key_file_path.push(models::params::EXIT_KEY_FILENAME);
        key_file_path
    };
    info!("Reading key from {}", path.to_string_lossy());
    let params = read_circuit_proving_parameters(&path).expect("Failed to read circuit parameters");
    info!("Read proving parameters, {} s", timer.elapsed().as_secs());
    params
}

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

    let (mut circuit_account_tree, account_id, token_id, balance) = {
        info!("Restoring state from db");
        let connection_pool = ConnectionPool::new();
        let storage = connection_pool
            .access_storage()
            .expect("Storage access failed");

        let token_id = if target_token_address == "ETH" {
            0
        } else {
            let tokens = storage.load_tokens().expect("Failed to load token");
            tokens
                .into_iter()
                .find(|(_, token)| token.address == target_token_address)
                .expect("Token not found")
                .0
        };

        let mut circuit_account_tree =
            CircuitAccountTree::new(models::params::account_tree_depth() as u32);
        let timer = Instant::now();

        let accounts = storage
            .load_verified_state()
            .expect("Failed to load verified state")
            .1;

        let mut target_account = None;
        for (id, account) in accounts {
            if account.address == target_account_address {
                target_account = Some((id, account.clone()));
            }
            account.get_balance(token_id);
            circuit_account_tree.insert(id, CircuitAccount::from(account));
        }

        let (account_id, balance) = target_account
            .map(|(id, acc)| (id, acc.get_balance(token_id)))
            .expect("Account not found");

        info!("State restored: {} s", timer.elapsed().as_secs());

        (circuit_account_tree, account_id, token_id, balance)
    };

    let parameters = read_parameters();
    info!("Generating proof");
    let timer = Instant::now();
    let (zksync_exit_circuit, public_input) =
        create_exit_circuit_with_public_input(&mut circuit_account_tree, account_id, token_id);
    let proof = create_random_full_baby_proof(zksync_exit_circuit, public_input, &parameters)
        .expect("Failed to generate proof");
    info!("Proof generated: {} s", timer.elapsed().as_secs());

    assert!(
        verify_full_baby_proof(&proof, &parameters).expect("Failed to verify proof"),
        "proof is invalid"
    );

    let proof_for_ethereum = encode_proof(&proof.proof);

    let proof_data = ExitProofData {
        token_id,
        owner: target_account_address,
        amount: balance,
        proof: proof_for_ethereum,
    };

    println!(
        "{}",
        serde_json::to_string(&proof_data).expect("proof data serialize")
    );
}
