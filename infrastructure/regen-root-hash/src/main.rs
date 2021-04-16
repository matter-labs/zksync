mod account;
mod hasher;
#[cfg(test)]
mod tests;
mod utils;

use std::env;
use utils::sign_update_message;
use zksync_circuit::witness::utils::fr_from_bytes;

use account::read_accounts;
use hasher::{get_state_root_hash, BALANCE_TREE_11, BALANCE_TREE_32};

fn main() {
    let args: Vec<String> = env::args().collect();

    let current_hash = args[1].clone();
    let accounts_file_path = args[2].clone();
    let balances_file_path = args[3].clone();
    let private_key = args[4].clone();

    let accounts = read_accounts(accounts_file_path, balances_file_path).unwrap();

    let current_hash_bytes = hex::decode(current_hash).unwrap();
    let current_hash_fr = fr_from_bytes(current_hash_bytes);

    let old_hash = get_state_root_hash(&accounts, &BALANCE_TREE_11);
    print!("OldHash: {} \n", old_hash.to_string());

    assert_eq!(
        old_hash, current_hash_fr,
        "The recalculated hash is not equal to the current one.\n"
    );

    let new_hash = get_state_root_hash(&accounts, &BALANCE_TREE_32);
    print!("NewHash: {} \n", new_hash.to_string());

    let signature = sign_update_message(private_key, old_hash, new_hash);
    print!("Sigтature: {}\n", signature);
}
