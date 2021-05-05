use num::BigUint;
use std::collections::HashMap;
use zksync_crypto::Fr;
use zksync_storage::chain::account::{
    records::{
        // To remove confusion with the StorageAccounts in the `account.rs`
        StorageAccount as ZkSyncStorageAccount,
        StorageBalance as ZkSyncStorageBalance,
    },
    restore_account::restore_account,
};
use zksync_storage::StorageProcessor;
use zksync_types::{Account, BlockNumber, Token, H256, U256};

pub async fn get_verified_block_number(
    storage: &mut StorageProcessor<'_>,
) -> anyhow::Result<BlockNumber> {
    let last_commited_block = storage
        .chain()
        .block_schema()
        .get_last_committed_block()
        .await?;
    let last_verifed_block = storage
        .chain()
        .block_schema()
        .get_last_verified_block()
        .await?;
    let last_verified_confirmed_block = storage
        .chain()
        .block_schema()
        .get_last_verified_confirmed_block()
        .await?;
    let pending_block_exists = storage
        .chain()
        .block_schema()
        .pending_block_exists()
        .await?;

    let the_block_after_last = storage
        .chain()
        .block_schema()
        .get_block(last_verified_confirmed_block + 1)
        .await?;

    assert!(the_block_after_last.is_none(), "The block is not last");

    assert!(
        last_commited_block == last_verifed_block,
        "There are committed, but not verified blocks"
    );
    assert!(
        last_commited_block == last_verified_confirmed_block,
        "There are verified unconfirmed blocks"
    );
    assert!(!pending_block_exists, "There exists a pending block");

    Ok(last_verified_confirmed_block)
}

pub async fn read_accounts_from_db() -> anyhow::Result<Vec<(i64, Account)>> {
    let mut storage_processor = StorageProcessor::establish_connection().await?;
    let mut transaction = storage_processor.start_transaction().await?;

    let stored_accounts = transaction
        .chain()
        .account_schema()
        .get_all_accounts()
        .await?;
    let stored_balances = transaction
        .chain()
        .account_schema()
        .get_all_balances()
        .await?;

    let mut accounts_balances: HashMap<i64, Vec<ZkSyncStorageBalance>> = HashMap::new();

    for stored_account in stored_accounts.iter() {
        accounts_balances.insert(stored_account.id, vec![]);
    }

    for stored_balance in stored_balances {
        let balances_vec = accounts_balances
            .get_mut(&stored_balance.account_id)
            .unwrap();
        balances_vec.push(stored_balance);
    }

    let mut accounts = vec![];
    for stored_account in stored_accounts {
        let account_balances = accounts_balances.get(&stored_account.id).unwrap();

        let account = restore_account(&stored_account, account_balances.to_vec());

        accounts.push((account.0 .0 as i64, account.1));
    }

    Ok(accounts)
}

pub async fn migrage_db_for_nft(past_root_hash: Fr, root_hash: Fr) -> anyhow::Result<()> {
    let mut storage_processor = StorageProcessor::establish_connection().await?;
    let mut transaction = storage_processor.start_transaction().await?;

    println!("Retrieving data about the last block...");
    let block_number = get_verified_block_number(&mut transaction).await?;
    let last_block = transaction
        .chain()
        .block_schema()
        .get_block(block_number)
        .await?
        .expect("The block does not exist");
    assert_eq!(
        last_block.new_root_hash, past_root_hash,
        "The past root hash is not correct"
    );

    println!("The last block's hash is correct. Setting the new root hash...");
    transaction
        .chain()
        .block_schema()
        .change_block_root_hash(block_number, root_hash)
        .await?;

    println!("The new root hash is set. Inserting nft account.");
    transaction
        .chain()
        .state_schema()
        .insert_nft_account(block_number)
        .await?;

    println!("Delete account tree cache for the last block.");
    transaction
        .chain()
        .block_schema()
        .reset_account_tree_cache(block_number)
        .await?;

    transaction.commit().await?;
    println!("DB migration complete.");

    Ok(())
}
