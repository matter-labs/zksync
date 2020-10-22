//! Generate exit proof for exodus mode given account and token
//! correct verified state should be present in the db (could be restored using `data-restore` module)

use serde::Serialize;
use std::time::Instant;
use structopt::StructOpt;
use zksync_crypto::proof::EncodedProofPlonk;
use zksync_storage::ConnectionPool;
use zksync_types::{AccountId, Address, TokenId, TokenLike};
use zksync_utils::BigUintSerdeWrapper;

#[derive(Serialize, Debug)]
struct ExitProofData {
    token_id: TokenId,
    account_id: AccountId,
    account_address: Address,
    amount: BigUintSerdeWrapper,
    proof: EncodedProofPlonk,
}

#[derive(StructOpt)]
#[structopt(
    name = "zkSync operator node",
    author = "Matter Labs",
    rename_all = "snake_case"
)]
struct Opt {
    /// Account id of the account
    #[structopt(long)]
    account_id: String,

    /// Token to withdraw - "ETH" or address of the ERC20 token
    #[structopt(long)]
    token: String,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let opt = Opt::from_args();

    let account_id = opt.account_id.parse::<AccountId>().unwrap();
    let token = TokenLike::parse(&opt.token);

    let timer = Instant::now();
    log::info!("Restoring state from db");
    let connection_pool = ConnectionPool::new(Some(1)).await;
    let mut storage = connection_pool
        .access_storage()
        .await
        .expect("Storage access failed");

    let token_id = storage
        .tokens_schema()
        .get_token(token)
        .await
        .expect("Db access fail")
        .expect(
            "Token not found. If you're addressing an ERC-20 token by it's symbol, \
                  it may not be available after data restore. Try using token address in that case",
        )
        .id;
    let address = storage
        .chain()
        .account_schema()
        .last_verified_state_for_account(account_id)
        .await
        .expect("DB access fail")
        .expect("Account not found in the db")
        .address;
    let accounts = storage
        .chain()
        .state_schema()
        .load_verified_state()
        .await
        .expect("Failed to load verified state")
        .1;

    log::info!("Restored state from db: {} s", timer.elapsed().as_secs());

    let (proof, amount) =
        zksync_prover::exit_proof::create_exit_proof(accounts, account_id, address, token_id)
            .expect("Failed to generate exit proof");

    let proof_data = ExitProofData {
        token_id,
        account_id,
        account_address: address,
        amount: amount.into(),
        proof,
    };

    println!("\n\n");
    println!("==========================");
    println!("Generating proof completed");
    println!("Below you can see the input data for the exit transaction on zkSync contract");
    println!("Look up the manuals of your desired smart wallet in order to know how to sign and send this transaction to the Ethereum");
    println!("==========================");

    println!("Exit transaction inputs:");

    println!(
        "{}",
        serde_json::to_string_pretty(&proof_data).expect("proof data serialize")
    );
}
