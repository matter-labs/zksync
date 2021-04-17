mod account;
mod hasher;
#[cfg(test)]
mod tests;
mod utils;

use structopt::StructOpt;
use utils::{fr_to_hex, sign_update_message, Params};
use zksync_circuit::witness::utils::fr_from_bytes;

use account::read_accounts;
use hasher::{get_state_root_hash, BALANCE_TREE_11, BALANCE_TREE_32};

fn main() {
    let params = Params::from_args();

    let accounts = read_accounts(params.accounts_dump, params.balances_dump).unwrap();

    let current_hash_bytes = hex::decode(params.current_root_hash).unwrap();
    let current_hash_fr = fr_from_bytes(current_hash_bytes);

    let old_hash = get_state_root_hash(&accounts, &BALANCE_TREE_11);
    println!("OldHash: {}", fr_to_hex(old_hash));

    assert_eq!(
        old_hash, current_hash_fr,
        "The recalculated hash is not equal to the current one."
    );

    let new_hash = get_state_root_hash(&accounts, &BALANCE_TREE_32);
    println!("NewHash: {}", fr_to_hex(new_hash));

    let signature = sign_update_message(params.private_key, old_hash, new_hash);
    println!("Signature: {}", signature);
}
