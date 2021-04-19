mod account;
mod hasher;
#[cfg(test)]
mod tests;
mod utils;

use structopt::StructOpt;
use utils::{fr_to_hex, sign_update_message, Params};
use zksync_circuit::witness::utils::fr_from_bytes;

use account::read_accounts;
use hasher::{
    get_state, verify_accounts_equal, verify_identical_trees, BALANCE_TREE_11, BALANCE_TREE_32,
};

fn main() {
    let params = Params::from_args();

    let accounts = read_accounts(params.accounts_dump, params.balances_dump).unwrap();

    let current_hash_bytes = hex::decode(params.current_root_hash).unwrap();
    let current_hash_fr = fr_from_bytes(current_hash_bytes);

    let old_tree = get_state(&accounts, &BALANCE_TREE_11);
    let old_hash = old_tree.root_hash();
    println!("OldHash: {}", fr_to_hex(old_hash));

    assert_eq!(
        old_hash, current_hash_fr,
        "The recalculated hash is not equal to the current one."
    );

    let new_tree = get_state(&accounts, &BALANCE_TREE_32);
    let new_hash = new_tree.root_hash();
    println!("NewHash: {}", fr_to_hex(new_hash));

    // Verify that each of the u32::MAX accounts has the same accounts in both trees
    verify_identical_trees(&old_tree, &new_tree, u32::MAX, verify_accounts_equal).unwrap();

    let signature = sign_update_message(params.private_key, old_hash, new_hash);
    println!("Signature: {}", signature);
}
