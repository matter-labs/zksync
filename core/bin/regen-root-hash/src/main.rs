mod account;
mod db_migrate;
mod hasher;
#[cfg(test)]
mod tests;
mod utils;

use account::{
    get_nft_circuit_account, read_accounts, verify_empty, CircuitAccountDepth11,
    CircuitAccountDepth32,
};

use structopt::StructOpt;
use utils::{fr_to_hex, sign_message};
use zksync_circuit::witness::utils::fr_from_bytes;

use hasher::{get_state, verify_accounts_equal, verify_identical_trees};
use zksync_crypto::params::NFT_STORAGE_ACCOUNT_ID;

use crate::db_migrate::migrage_db_for_nft;
use crate::{db_migrate::read_accounts_from_db, utils::get_message_to_sign};

#[derive(Debug, StructOpt)]
pub struct Params {
    /// The current root hash (balance subtree depth 11)
    #[structopt(short = "h", env = "CURRENT_ROOT_HASH")]
    pub current_root_hash: String,

    /// A flag to tell that we want to migrate the db
    #[structopt(short = "d")]
    pub db_migrate: bool,

    /// The path to the JSON dump of the accounts table
    #[structopt(short = "a", env = "ACCOUNTS_DUMP")]
    pub accounts_dump: Option<String>,

    /// The path to the JSON dump of the accounts table
    #[structopt(short = "b", env = "BALANCES_DUMP")]
    pub balances_dump: Option<String>,

    /// The private key of the signer
    #[structopt(short = "p", env = "PRIVATE_KEY")]
    pub private_key: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let params = Params::from_args();

    let accounts = if params.db_migrate {
        read_accounts_from_db().await?
    } else {
        let accounts_dump = params
            .accounts_dump
            .expect("The accounts dump should be provided if we are not interacting wirh db");
        let balances_dump = params
            .balances_dump
            .expect("The balances dump should be provided if we are not interacting wirh db");
        read_accounts(accounts_dump, balances_dump)?
    };

    let current_hash_bytes = hex::decode(params.current_root_hash).unwrap();
    let current_hash_fr = fr_from_bytes(current_hash_bytes);

    let old_tree = get_state::<CircuitAccountDepth11>(&accounts);

    // Verifying that the nft account is empty
    verify_empty(NFT_STORAGE_ACCOUNT_ID.0, &old_tree).unwrap();

    let old_hash = old_tree.root_hash();
    println!("OldHash: 0x{}", fr_to_hex(old_hash));

    assert_eq!(
        old_hash, current_hash_fr,
        "The recalculated hash is not equal to the current one."
    );

    let mut new_tree = get_state::<CircuitAccountDepth32>(&accounts);

    // Verify that each of the u32::MAX accounts has the same accounts in both trees
    verify_identical_trees(&old_tree, &new_tree, u32::MAX, verify_accounts_equal).unwrap();

    // The new tree will also contain the NFT_STORAGE_ACCOUNT
    let nft_account = get_nft_circuit_account();
    new_tree.insert(NFT_STORAGE_ACCOUNT_ID.0, nft_account);
    let new_hash = new_tree.root_hash();
    println!("NewHash: 0x{}", fr_to_hex(new_hash));

    if params.db_migrate {
        println!("Migrating the database to enable NFTs");
        migrage_db_for_nft(old_hash, new_hash).await?;
    } else {
        let message_to_sign = get_message_to_sign(old_hash, new_hash);
        println!("\nSigning prefixed message: {}", message_to_sign);

        let private_key = params.private_key.expect("Private key should be supplied");
        let signature = sign_message(private_key, message_to_sign);
        println!("\nSignature: 0x{}", signature);
    }

    Ok(())
}
