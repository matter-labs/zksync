use serde::Serialize;
use std::collections::HashMap;
use std::convert::TryInto;
use zksync_crypto::{
    params::{MIN_NFT_TOKEN_ID, NFT_STORAGE_ACCOUNT_ADDRESS, NFT_STORAGE_ACCOUNT_ID, NFT_TOKEN_ID},
    Fr,
};
use zksync_storage::StorageProcessor;
use zksync_storage::{
    chain::account::{
        records::{
            StorageAccountCreation,
            StorageAccountUpdate,
            // To remove confusion with the StorageBalance in the `account.rs`
            StorageBalance as DbStorageBalance,
        },
        restore_account::restore_account,
    },
    diff::StorageAccountDiff,
    BigDecimal,
};
use zksync_types::{Account, AccountUpdate, BlockNumber, Token};

use crate::{account::CircuitAccountWrapper, hasher::CustomMerkleTree, utils::fr_to_hex};

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

    let mut accounts_balances: HashMap<i64, Vec<DbStorageBalance>> = HashMap::new();

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

pub async fn add_nft_special_token(storage: &mut StorageProcessor<'_>) -> anyhow::Result<()> {
    storage
        .tokens_schema()
        .store_token(Token {
            id: NFT_TOKEN_ID,
            symbol: "SPECIAL".to_string(),
            address: *NFT_STORAGE_ACCOUNT_ADDRESS,
            decimals: 18,
            is_nft: true, // TODO: ZKS-635
        })
        .await?;
    Ok(())
}

pub async fn commit_nft_special_account(
    storage: &mut StorageProcessor<'_>,
    block_number: BlockNumber,
    update_order_id: usize,
) -> anyhow::Result<Account> {
    let (mut special_account, db_create_special_account) =
        Account::create_account(NFT_STORAGE_ACCOUNT_ID, *NFT_STORAGE_ACCOUNT_ADDRESS);
    special_account.set_balance(NFT_TOKEN_ID, num::BigUint::from(MIN_NFT_TOKEN_ID));

    let db_set_special_account_balance = AccountUpdate::UpdateBalance {
        old_nonce: special_account.nonce,
        new_nonce: special_account.nonce,
        balance_update: (
            NFT_TOKEN_ID,
            num::BigUint::from(0u64),
            num::BigUint::from(MIN_NFT_TOKEN_ID),
        ),
    };

    storage
        .chain()
        .state_schema()
        .commit_state_update(
            block_number,
            &[
                db_create_special_account[0usize].clone(),
                (NFT_STORAGE_ACCOUNT_ID, db_set_special_account_balance),
            ],
            update_order_id,
        )
        .await?;

    Ok(special_account)
}

pub async fn apply_nft_storage_account(
    storage: &mut StorageProcessor<'_>,
    special_account: Account,
    block_number: BlockNumber,
    update_order_id: usize,
) -> anyhow::Result<()> {
    let account_id: i64 = NFT_STORAGE_ACCOUNT_ID.0.try_into().unwrap();
    let nonce: i64 = special_account.nonce.0.try_into().unwrap();
    let block_number: i64 = block_number.0.try_into().unwrap();
    let update_order_id: i32 = update_order_id.try_into().unwrap();
    let coin_id: i32 = NFT_TOKEN_ID.0.try_into().unwrap();

    let storage_account_creation = StorageAccountCreation {
        account_id,
        is_create: true,
        block_number,
        address: NFT_STORAGE_ACCOUNT_ADDRESS.as_bytes().to_vec(),
        nonce,
        update_order_id,
    };

    let storage_balance_update = StorageAccountUpdate {
        // This value is not used for our pursposes and will not be stored anywhere
        // so can put whatever we want here
        balance_update_id: 0,
        account_id,
        block_number,
        coin_id,
        old_balance: BigDecimal::from(0u64),
        new_balance: BigDecimal::from(MIN_NFT_TOKEN_ID),
        old_nonce: nonce,
        new_nonce: nonce,
        update_order_id: update_order_id + 1,
    };

    let create_diff = StorageAccountDiff::Create(storage_account_creation);
    let upd_balance_diff = StorageAccountDiff::BalanceUpdate(storage_balance_update);

    storage
        .chain()
        .state_schema()
        .apply_storage_account_diff(create_diff)
        .await?;
    storage
        .chain()
        .state_schema()
        .apply_storage_account_diff(upd_balance_diff)
        .await?;

    Ok(())
}

pub async fn insert_nft_account(
    storage: &mut StorageProcessor<'_>,
    block_number: BlockNumber,
) -> anyhow::Result<()> {
    let mut transaction = storage.start_transaction().await?;

    add_nft_special_token(&mut transaction).await?;

    // This number is only used for sorting when applying the block
    // Since we are overridining the existing block
    // we could enter here any number we want
    let update_order_id = 1000;

    let special_account =
        commit_nft_special_account(&mut transaction, block_number, update_order_id).await?;

    // Applying account

    apply_nft_storage_account(
        &mut transaction,
        special_account,
        block_number,
        update_order_id,
    )
    .await?;

    transaction.commit().await?;

    Ok(())
}

pub async fn migrage_db_for_nft<T: CircuitAccountWrapper>(
    past_root_hash: Fr,
    new_tree: CustomMerkleTree<T>,
) -> anyhow::Result<()> {
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

    let new_root_hash = new_tree.root_hash();

    transaction
        .chain()
        .block_schema()
        .change_block_root_hash(block_number, new_root_hash)
        .await?;

    println!("The new root hash is set. Inserting nft account.");
    insert_nft_account(&mut transaction, block_number).await?;

    println!("Delete account tree cache for the last block.");
    transaction
        .chain()
        .block_schema()
        .reset_account_tree_cache(block_number)
        .await?;

    let tree_cache = new_tree.get_internals();
    let tree_cache = serde_json::to_value(tree_cache)?;
    transaction
        .chain()
        .block_schema()
        .store_account_tree_cache(block_number, tree_cache)
        .await?;

    transaction.commit().await?;
    println!("DB migration complete.");

    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StoredBlockInfo {
    block_number: u32,
    priority_operations: u64,
    pending_onchain_operations_hash: String,
    timestamp: u64,
    state_hash: String,
    commitment: String,
}

pub async fn get_last_block_info() -> anyhow::Result<String> {
    let mut storage_processor = StorageProcessor::establish_connection().await?;

    let last_block_number = get_verified_block_number(&mut storage_processor).await?;

    let block = storage_processor
        .chain()
        .block_schema()
        .get_block(last_block_number)
        .await?
        .unwrap();

    let priority_op_hash = block
        .get_onchain_operations_block_info()
        .1
        .as_bytes()
        .to_vec();
    let priority_op_hash = hex::encode(&priority_op_hash);

    let commitment_str = hex::encode(block.block_commitment.as_bytes());

    let last_block_info = StoredBlockInfo {
        block_number: *block.block_number,
        priority_operations: block.number_of_processed_prior_ops(),
        pending_onchain_operations_hash: format!("0x{}", priority_op_hash),
        timestamp: block.timestamp,
        state_hash: format!("0x{}", fr_to_hex(block.new_root_hash)),
        commitment: format!("0x{}", commitment_str),
    };

    let info_str = serde_json::ser::to_string(&last_block_info)?;

    Ok(info_str)
}
