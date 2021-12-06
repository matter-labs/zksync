use std::time::Instant;

// External uses
// Workspace uses
use zksync_crypto::{
    ff,
    params::{MIN_NFT_TOKEN_ID, NFT_STORAGE_ACCOUNT_ADDRESS, NFT_STORAGE_ACCOUNT_ID, NFT_TOKEN_ID},
};
use zksync_state::state::ZkSyncState;
use zksync_storage::ConnectionPool;
use zksync_types::{Account, AccountId, AccountUpdate, Address, BlockNumber, Token, TokenKind};
// Local uses

pub async fn create_genesis_block(pool: ConnectionPool, fee_account_address: &Address) {
    let start = Instant::now();
    let mut storage = pool
        .access_storage()
        .await
        .expect("db connection failed for statekeeper");
    let mut transaction = storage
        .start_transaction()
        .await
        .expect("unable to create db transaction in statekeeper");

    let (last_committed, mut accounts) = transaction
        .chain()
        .state_schema()
        .load_committed_state(None)
        .await
        .expect("db failed");

    assert!(
        *last_committed == 0 && accounts.is_empty(),
        "db should be empty"
    );

    vlog::info!("Adding special token");
    transaction
        .tokens_schema()
        .store_token(Token::new(
            NFT_TOKEN_ID,
            *NFT_STORAGE_ACCOUNT_ADDRESS,
            "SPECIAL",
            18,
            TokenKind::NFT,
        ))
        .await
        .expect("failed to store special token");
    vlog::info!("Special token added");

    let fee_account = Account::default_with_address(fee_account_address);
    let db_create_fee_account = AccountUpdate::Create {
        address: *fee_account_address,
        nonce: fee_account.nonce,
    };
    accounts.insert(AccountId(0), fee_account);

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
    accounts.insert(NFT_STORAGE_ACCOUNT_ID, special_account);

    transaction
        .chain()
        .state_schema()
        .commit_state_update(
            BlockNumber(0),
            &[
                (AccountId(0), db_create_fee_account),
                db_create_special_account[0].clone(),
                (NFT_STORAGE_ACCOUNT_ID, db_set_special_account_balance),
            ],
            0,
        )
        .await
        .expect("db fail");
    transaction
        .chain()
        .state_schema()
        .apply_state_update(BlockNumber(0))
        .await
        .expect("db fail");

    let state = ZkSyncState::from_acc_map(accounts, last_committed + 1);
    let root_hash = state.root_hash();
    transaction
        .chain()
        .block_schema()
        .save_genesis_block(root_hash)
        .await
        .expect("db fail");

    transaction
        .commit()
        .await
        .expect("Unable to commit transaction in statekeeper");
    vlog::info!("Genesis block created, state: {}", state.root_hash());

    // Below we are intentionally using `println`, because during genesis we parse the genesis root from
    // the server output in order to save it into the config file.
    // See `server.genesis()` in the `zk` tool for details.
    // TODO: Find a better and a more intuitive approach (ZKS-816).
    let genesis_root = format!("CONTRACTS_GENESIS_ROOT=0x{}", ff::to_hex(&root_hash));
    println!("{}", &genesis_root);

    metrics::histogram!("state_keeper.create_genesis_block", start.elapsed());
}
