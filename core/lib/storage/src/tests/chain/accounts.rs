// External imports
use num::{BigUint, Zero};
// Workspace imports
use zksync_crypto::params::{MIN_NFT_TOKEN_ID, NFT_TOKEN_ID};
use zksync_types::{
    aggregated_operations::AggregatedActionType, helpers::apply_updates, AccountId, AccountMap,
    AccountUpdate, Address, BlockNumber, Nonce, Token, TokenId, TokenKind,
};
// Local imports
use super::block::apply_random_updates;
use crate::chain::operations::OperationsSchema;
use crate::test_data::{gen_sample_block, gen_unique_aggregated_operation, generate_nft};
use crate::tests::{create_rng, db_test, ACCOUNT_MUTEX};
use crate::{
    chain::{
        account::{records::EthAccountType, AccountSchema},
        block::BlockSchema,
        state::StateSchema,
    },
    QueryResult, StorageProcessor,
};

/// The save/load routine for EthAccountType
#[db_test]
async fn eth_account_type(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // check that function returns None by default
    let non_existent = AccountSchema(&mut storage)
        .account_type_by_id(AccountId(18))
        .await?;
    assert!(non_existent.is_none());

    // store account type and then load it
    AccountSchema(&mut storage)
        .set_account_type(AccountId(18), EthAccountType::CREATE2)
        .await?;
    let loaded = AccountSchema(&mut storage)
        .account_type_by_id(AccountId(18))
        .await?;
    assert!(matches!(loaded, Some(EthAccountType::CREATE2)));

    Ok(())
}

/// Checks that stored accounts can be obtained once they're committed.
#[db_test]
async fn stored_accounts(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let _lock = ACCOUNT_MUTEX.lock().await;
    let mut rng = create_rng();

    let block_size = 100;

    let (last_finalized, _) = AccountSchema(&mut storage)
        .account_and_last_block(AccountId(1))
        .await?;
    let last_committed = AccountSchema(&mut storage)
        .last_committed_block_with_update_for_acc(AccountId(1), BlockNumber(last_finalized as u32))
        .await?;
    assert_eq!(last_finalized, 0);
    assert_eq!(*last_committed, 0);

    // Create several accounts.
    let accounts = AccountMap::default();

    // Create several accounts.
    let (mut accounts_block, mut updates_block) = apply_random_updates(accounts, &mut rng);

    let mut nft_updates = vec![];
    accounts_block
        .iter()
        .enumerate()
        .for_each(|(id, (account_id, account))| {
            nft_updates.append(&mut generate_nft(
                *account_id,
                account,
                accounts_block.len() as u32 + id as u32,
                &mut rng,
            ));
        });
    apply_updates(&mut accounts_block, nft_updates.clone());

    updates_block.extend(nft_updates);
    // Execute and commit block with them.
    // Also store account updates.
    BlockSchema(&mut storage)
        .save_full_block(gen_sample_block(
            BlockNumber(1),
            block_size,
            Default::default(),
        ))
        .await?;
    StateSchema(&mut storage)
        .commit_state_update(BlockNumber(1), &updates_block, 0)
        .await?;

    // Get the accounts by their addresses.
    for (account_id, account) in accounts_block.iter() {
        let mut account = account.clone();

        let (last_finalized, _) = AccountSchema(&mut storage)
            .account_and_last_block(*account_id)
            .await?;
        let last_committed = AccountSchema(&mut storage)
            .last_committed_block_with_update_for_acc(
                *account_id,
                BlockNumber(last_finalized as u32),
            )
            .await?;
        assert_eq!(last_finalized, 0);
        assert_eq!(*last_committed, 1);

        let account_state = AccountSchema(&mut storage)
            .account_state_by_address(account.address)
            .await?;

        // Check that committed state is available, but verified is not.
        assert!(
            account_state.committed.is_some(),
            "No committed state for account"
        );
        assert!(
            account_state.verified.is_none(),
            "Block is not verified, but account has a verified state"
        );

        // Compare the obtained stored account with expected one.
        let (got_account_id, got_account) = account_state.committed.unwrap();

        // We have to copy this field, since it is not initialized by default.
        account.pub_key_hash = got_account.pub_key_hash;

        assert_eq!(got_account_id, *account_id);
        assert_eq!(got_account, account);

        // Also check `last_committed_state_for_account` method.
        assert_eq!(
            AccountSchema(&mut storage)
                .last_committed_state_for_account(*account_id)
                .await?
                .1,
            Some(got_account)
        );

        // Check account address and ID getters.
        assert_eq!(
            AccountSchema(&mut storage)
                .account_address_by_id(*account_id)
                .await?,
            Some(account.address)
        );
        assert_eq!(
            AccountSchema(&mut storage)
                .account_id_by_address(account.address)
                .await?,
            Some(*account_id)
        );
    }

    // Now add a proof, verify block and apply a state update.
    OperationsSchema(&mut storage)
        .store_aggregated_action(gen_unique_aggregated_operation(
            BlockNumber(1),
            AggregatedActionType::ExecuteBlocks,
            block_size,
        ))
        .await?;
    StateSchema(&mut storage)
        .apply_state_update(BlockNumber(1))
        .await?;

    // After that all the accounts should have a verified state.
    for (account_id, account) in accounts_block {
        let (last_finalized, _) = AccountSchema(&mut storage)
            .account_and_last_block(account_id)
            .await?;
        let last_committed = AccountSchema(&mut storage)
            .last_committed_block_with_update_for_acc(
                account_id,
                BlockNumber(last_finalized as u32),
            )
            .await?;
        assert_eq!(last_finalized, 1);
        assert_eq!(*last_committed, 1);

        let account_state = AccountSchema(&mut storage)
            .account_state_by_id(account_id)
            .await?;

        assert!(
            account_state.committed.is_some(),
            "No committed state for account"
        );
        assert!(
            account_state.verified.is_some(),
            "No verified state for the account"
        );

        assert!(
            !account_state.committed.unwrap().1.minted_nfts.is_empty(),
            "Some NFTs should be minted by account"
        );

        // Compare the obtained stored account with expected one.
        let (got_account_id, got_account) = account_state.verified.unwrap();

        assert_eq!(got_account_id, account_id);
        assert_eq!(got_account, account);

        // Also check `last_verified_state_for_account` method.
        assert_eq!(
            AccountSchema(&mut storage)
                .last_verified_state_for_account(account_id)
                .await?,
            Some(got_account)
        );
    }

    Ok(())
}

#[db_test]
async fn test_get_balance(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let _lock = ACCOUNT_MUTEX.lock().await;
    let address = Address::random();
    let updates1 = vec![
        (
            AccountId(1),
            AccountUpdate::Create {
                address,
                nonce: Nonce(0),
            },
        ),
        (
            AccountId(2),
            AccountUpdate::Create {
                address: Address::random(),
                nonce: Nonce(0),
            },
        ),
        (
            AccountId(3),
            AccountUpdate::Create {
                address: Address::random(),
                nonce: Nonce(0),
            },
        ),
        (
            AccountId(1),
            AccountUpdate::UpdateBalance {
                old_nonce: Nonce(0),
                new_nonce: Nonce(1),
                balance_update: (TokenId(0), BigUint::zero(), BigUint::from(100u32)),
            },
        ),
    ];
    let updates2 = vec![
        (
            AccountId(1),
            AccountUpdate::UpdateBalance {
                old_nonce: Nonce(1),
                new_nonce: Nonce(2),
                balance_update: (TokenId(0), BigUint::from(100u32), BigUint::from(200u32)),
            },
        ),
        (
            AccountId(1),
            AccountUpdate::UpdateBalance {
                old_nonce: Nonce(2),
                new_nonce: Nonce(3),
                balance_update: (TokenId(0), BigUint::from(200u32), BigUint::from(300u32)),
            },
        ),
        (
            AccountId(1),
            AccountUpdate::UpdateBalance {
                old_nonce: Nonce(3),
                new_nonce: Nonce(4),
                balance_update: (TokenId(1), BigUint::zero(), BigUint::from(10000u32)),
            },
        ),
    ];
    storage
        .chain()
        .state_schema()
        .commit_state_update(BlockNumber(2), &updates1, 0)
        .await?;
    storage
        .chain()
        .state_schema()
        .commit_state_update(BlockNumber(3), &updates2, 0)
        .await?;

    let balance01 = storage
        .chain()
        .account_schema()
        .get_account_balance_for_block(address, BlockNumber(1), TokenId(0))
        .await?;
    let balance02 = storage
        .chain()
        .account_schema()
        .get_account_balance_for_block(address, BlockNumber(2), TokenId(0))
        .await?;
    let balance03 = storage
        .chain()
        .account_schema()
        .get_account_balance_for_block(address, BlockNumber(3), TokenId(0))
        .await?;
    let balance04 = storage
        .chain()
        .account_schema()
        .get_account_balance_for_block(address, BlockNumber(4), TokenId(0))
        .await?;
    let balance14 = storage
        .chain()
        .account_schema()
        .get_account_balance_for_block(address, BlockNumber(4), TokenId(1))
        .await?;
    assert_eq!(balance01, BigUint::zero());
    assert_eq!(balance02, BigUint::from(100u32));
    assert_eq!(balance03, BigUint::from(300u32));
    assert_eq!(balance04, BigUint::from(300u32));
    assert_eq!(balance14, BigUint::from(10000u32));

    Ok(())
}

#[db_test]
async fn test_get_account_nft_balance(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let address = Address::random();
    let nft_id = TokenId(MIN_NFT_TOKEN_ID + 100);

    storage
        .tokens_schema()
        .store_or_update_token(Token {
            id: nft_id,
            address: Address::random(),
            symbol: "NFT".to_string(),
            decimals: 0,
            kind: TokenKind::NFT,
            is_nft: true,
        })
        .await?;
    storage
        .tokens_schema()
        .store_or_update_token(Token {
            id: NFT_TOKEN_ID,
            address: Address::random(),
            symbol: "SPECIAL".to_string(),
            decimals: 0,
            kind: TokenKind::NFT,
            is_nft: true,
        })
        .await?;

    // Checks that nonexistent account has zero nft balance.
    let nft_balance0 = storage
        .chain()
        .account_schema()
        .get_account_nft_balance(address)
        .await?;
    assert_eq!(nft_balance0, 0u32);

    let updates1 = vec![
        (
            AccountId(1),
            AccountUpdate::Create {
                address,
                nonce: Nonce(0),
            },
        ),
        (
            AccountId(1),
            AccountUpdate::UpdateBalance {
                old_nonce: Nonce(0),
                new_nonce: Nonce(1),
                balance_update: (nft_id, BigUint::zero(), BigUint::from(1u32)),
            },
        ),
    ];
    storage
        .chain()
        .state_schema()
        .commit_state_update(BlockNumber(1), &updates1, 0)
        .await?;
    storage
        .chain()
        .state_schema()
        .apply_state_update(BlockNumber(1))
        .await?;

    // Checks that nft balance has changed after applying state update.
    let nft_balance1 = storage
        .chain()
        .account_schema()
        .get_account_nft_balance(address)
        .await?;
    assert_eq!(nft_balance1, 1u32);

    let updates2 = vec![(
        AccountId(1),
        AccountUpdate::UpdateBalance {
            old_nonce: Nonce(0),
            new_nonce: Nonce(1),
            balance_update: (NFT_TOKEN_ID, BigUint::zero(), BigUint::from(1u32)),
        },
    )];
    storage
        .chain()
        .state_schema()
        .commit_state_update(BlockNumber(2), &updates2, updates1.len())
        .await?;
    storage
        .chain()
        .state_schema()
        .apply_state_update(BlockNumber(2))
        .await?;

    // Checks that nft balance hasn't changed after updating balance of special token.
    let nft_balance2 = storage
        .chain()
        .account_schema()
        .get_account_nft_balance(address)
        .await?;
    assert_eq!(nft_balance2, 1u32);

    Ok(())
}

#[db_test]
async fn test_get_nft_owner(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let account_id1 = AccountId(1);
    let account_id2 = AccountId(2);
    let address1 = Address::random();
    let address2 = Address::random();
    let nft_id = TokenId(MIN_NFT_TOKEN_ID + 100);

    // Checks that there is no owner for nonexistent nft.
    let owner = storage
        .chain()
        .account_schema()
        .get_nft_owner(nft_id)
        .await?;
    assert!(owner.is_none());

    let updates1 = vec![
        (
            account_id1,
            AccountUpdate::Create {
                address: address1,
                nonce: Nonce(0),
            },
        ),
        (
            account_id1,
            AccountUpdate::UpdateBalance {
                old_nonce: Nonce(0),
                new_nonce: Nonce(1),
                balance_update: (nft_id, BigUint::zero(), BigUint::from(1u32)),
            },
        ),
    ];
    storage
        .tokens_schema()
        .store_or_update_token(Token {
            id: nft_id,
            address: Address::random(),
            symbol: "NFT".to_string(),
            decimals: 0,
            kind: TokenKind::NFT,
            is_nft: true,
        })
        .await?;
    storage
        .chain()
        .state_schema()
        .commit_state_update(BlockNumber(1), &updates1, 0)
        .await?;
    storage
        .chain()
        .state_schema()
        .apply_state_update(BlockNumber(1))
        .await?;

    // Checks that owner is correct after first block.
    let owner = storage
        .chain()
        .account_schema()
        .get_nft_owner(nft_id)
        .await?;
    assert_eq!(owner.unwrap(), account_id1);

    let updates2 = vec![
        (
            account_id2,
            AccountUpdate::Create {
                address: address2,
                nonce: Nonce(0),
            },
        ),
        (
            account_id1,
            AccountUpdate::UpdateBalance {
                old_nonce: Nonce(1),
                new_nonce: Nonce(2),
                balance_update: (nft_id, BigUint::from(1u32), BigUint::zero()),
            },
        ),
        (
            account_id2,
            AccountUpdate::UpdateBalance {
                old_nonce: Nonce(0),
                new_nonce: Nonce(1),
                balance_update: (nft_id, BigUint::zero(), BigUint::from(1u32)),
            },
        ),
    ];
    storage
        .chain()
        .state_schema()
        .commit_state_update(BlockNumber(2), &updates2, updates1.len())
        .await?;
    storage
        .chain()
        .state_schema()
        .apply_state_update(BlockNumber(2))
        .await?;

    // Checks that owner is correct after second block.
    let owner = storage
        .chain()
        .account_schema()
        .get_nft_owner(nft_id)
        .await?;
    assert_eq!(owner.unwrap(), account_id2);

    Ok(())
}
