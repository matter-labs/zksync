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
use utils::{fr_to_hex, get_tx_data};
use zksync_circuit::witness::utils::fr_from_bytes;

use hasher::{get_state, verify_accounts_equal, verify_identical_trees};
use zksync_crypto::params::NFT_STORAGE_ACCOUNT_ID;

use crate::db_migrate::read_accounts_from_db;
use crate::db_migrate::{get_last_block_info, migrage_db_for_nft};

#[derive(Debug, StructOpt)]
pub struct Params {
    /// The current root hash (balance subtree depth 11)
    #[structopt(short = "h", env = "CURRENT_ROOT_HASH")]
    pub current_root_hash: Option<String>,

    /// A flag to tell that we want to migrate the db
    #[structopt(long = "db-migrate")]
    pub db_migrate: bool,

    /// A flag that indicates that the new tree will not be
    /// double-checked. Shoult NOT be used in production
    #[structopt(long = "no-double-check")]
    pub no_double_check: bool,

    /// Only retrieve StoredBlockInfo about the last block
    #[structopt(long = "last-block-info")]
    pub last_block_info: bool,

    /// The path to the JSON dump of the accounts table
    #[structopt(short = "a", env = "ACCOUNTS_DUMP")]
    pub accounts_dump: Option<String>,

    /// The path to the JSON dump of the accounts table
    #[structopt(short = "b", env = "BALANCES_DUMP")]
    pub balances_dump: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let params = Params::from_args();

    if params.last_block_info {
        let last_block_info = get_last_block_info().await?;
        println!("{}", last_block_info);
        return Ok(());
    }

    let current_root_hash = params.current_root_hash.unwrap();
    // Removing 0x...
    let current_root_hash = current_root_hash[2..].to_owned();

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

    let current_hash_bytes = hex::decode(current_root_hash).unwrap();
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

    if params.no_double_check {
        println!("Skipping re-verification of the trees. This should not be used in production.");
    } else {
        // Verify that each of the u32::MAX accounts has the same accounts in both trees
        verify_identical_trees(&old_tree, &new_tree, u32::MAX, verify_accounts_equal).unwrap();
    }

    // The new tree will also contain the NFT_STORAGE_ACCOUNT
    let nft_account = get_nft_circuit_account();
    new_tree.insert(NFT_STORAGE_ACCOUNT_ID.0, nft_account);
    let new_hash = new_tree.root_hash();
    println!("NewHash: 0x{}", fr_to_hex(new_hash));

    if params.db_migrate {
        println!("Migrating the database to enable NFTs");
        migrage_db_for_nft(old_hash, new_tree).await?;
    } else {
        let calldata = get_tx_data(old_hash, new_hash);
        println!(
            "The calldata of the call to regenesis multisig is 0x{}",
            calldata
        );
    }

    Ok(())
}
