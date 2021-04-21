mod account;
mod hasher;
#[cfg(test)]
mod tests;
mod utils;

use account::{
    get_nft_account, read_accounts, verify_empty, CircuitAccountDepth11, CircuitAccountDepth32,
};

use structopt::StructOpt;
use utils::{fr_to_hex, sign_update_message};
use zksync_circuit::witness::utils::fr_from_bytes;

use hasher::{get_state, verify_accounts_equal, verify_identical_trees};
use zksync_crypto::params::NFT_STORAGE_ACCOUNT_ID;

#[derive(StructOpt)]
pub struct Params {
    /// The current root hash (balance subtree depth 11)
    #[structopt(short = "h")]
    pub current_root_hash: String,

    /// The path to the JSON dump of the accounts table
    #[structopt(short = "a")]
    pub accounts_dump: String,

    /// The path to the JSON dump of the accounts table
    #[structopt(short = "b")]
    pub balances_dump: String,

    /// The private key of the signer
    #[structopt(short = "pk")]
    pub private_key: String,
}

fn main() {
    let params = Params::from_args();

    let accounts = read_accounts(params.accounts_dump, params.balances_dump).unwrap();

    let current_hash_bytes = hex::decode(params.current_root_hash).unwrap();
    let current_hash_fr = fr_from_bytes(current_hash_bytes);

    let old_tree = get_state::<CircuitAccountDepth11>(&accounts);

    // Verifying that the nft account is empty
    verify_empty(NFT_STORAGE_ACCOUNT_ID.0, &old_tree).unwrap();

    let old_hash = old_tree.root_hash();
    println!("OldHash: {}", fr_to_hex(old_hash));

    assert_eq!(
        old_hash, current_hash_fr,
        "The recalculated hash is not equal to the current one."
    );

    let mut new_tree = get_state::<CircuitAccountDepth32>(&accounts);

    // Verify that each of the u32::MAX accounts has the same accounts in both trees
    verify_identical_trees(&old_tree, &new_tree, u32::MAX, verify_accounts_equal).unwrap();

    // The new tree will also contain the NFT_STORAGE_ACCOUNT
    let nft_account = get_nft_account();
    new_tree.insert(NFT_STORAGE_ACCOUNT_ID.0, nft_account);
    let new_hash = new_tree.root_hash();
    println!("NewHash: {}", fr_to_hex(new_hash));

    let signature = sign_update_message(params.private_key, old_hash, new_hash);
    println!("Signature: {}", signature);
}
