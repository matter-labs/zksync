// Built-in imports
use std::str::FromStr;
// External imports
use chrono::Utc;
use num::{rational::Ratio, BigUint};
// Workspace imports
use zksync_test_account::ZkSyncAccount;
use zksync_types::{
    tokens::TokenMarketVolume, AccountId, Address, BlockNumber, ExecutedOperations, ExecutedTx,
    Token, TokenId, TokenLike, TokenPrice, WithdrawNFTOp, ZkSyncOp, H256,
};
use zksync_utils::{big_decimal_to_ratio, ratio_to_big_decimal};
// Local imports
use crate::tests::db_test;
use crate::{
    chain::account::records::StorageMintNFTUpdate,
    diff::StorageAccountDiff,
    tokens::{TokensSchema, STORED_USD_PRICE_PRECISION},
    QueryResult, StorageProcessor,
};
use zksync_crypto::params::MIN_NFT_TOKEN_ID;

/// Verifies the token save & load mechanism.
#[db_test]
async fn tokens_storage(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // There should be only Ethereum main token by default.
    assert_eq!(storage.tokens_schema().get_count().await?, 0);
    let tokens = TokensSchema(&mut storage)
        .load_tokens()
        .await
        .expect("Load tokens query failed");
    assert_eq!(tokens.len(), 1);
    let eth_token = Token {
        id: TokenId(0),
        address: "0000000000000000000000000000000000000000".parse().unwrap(),
        symbol: "ETH".into(),
        decimals: 18,
        is_nft: false,
    };
    assert_eq!(tokens[&TokenId(0)], eth_token);

    // Add two tokens.
    let token_a = Token {
        id: TokenId(1),
        address: "0000000000000000000000000000000000000001".parse().unwrap(),
        symbol: "ABC".into(),
        decimals: 9,
        is_nft: false,
    };
    let token_b = Token {
        id: TokenId(2),
        address: "0000000000000000000000000000000000000002".parse().unwrap(),
        symbol: "DEF".into(),
        decimals: 6,
        is_nft: false,
    };
    let nft = Token {
        id: TokenId(MIN_NFT_TOKEN_ID),
        address: "0000000000000000000000000000000000000005".parse().unwrap(),
        symbol: "NFT".into(),
        decimals: 0,
        is_nft: true,
    };

    TokensSchema(&mut storage)
        .store_or_update_token(nft.clone())
        .await
        .expect("Store tokens query failed");

    TokensSchema(&mut storage)
        .store_or_update_token(token_a.clone())
        .await
        .expect("Store tokens query failed");
    TokensSchema(&mut storage)
        .store_or_update_token(token_b.clone())
        .await
        .expect("Store tokens query failed");
    // The count is updated.
    assert_eq!(storage.tokens_schema().get_count().await?, 2);

    // Load tokens again.
    let tokens = TokensSchema(&mut storage)
        .load_tokens()
        .await
        .expect("Load tokens query failed");

    assert_eq!(tokens.len(), 3);
    assert_eq!(tokens[&eth_token.id], eth_token);
    assert_eq!(tokens[&token_a.id], token_a);
    assert_eq!(tokens[&token_b.id], token_b);

    let token_b_by_id = TokensSchema(&mut storage)
        .get_token(TokenLike::Id(token_b.id))
        .await
        .expect("get token query failed")
        .expect("token by id not found");
    assert_eq!(token_b, token_b_by_id);

    let token_b_by_address = TokensSchema(&mut storage)
        .get_token(TokenLike::Address(token_b.address))
        .await
        .expect("get token query failed")
        .expect("token by address not found");
    assert_eq!(token_b, token_b_by_address);

    let token_b_by_symbol = TokensSchema(&mut storage)
        .get_token(TokenLike::Symbol(token_b.symbol.clone()))
        .await
        .expect("get token query failed")
        .expect("token by symbol not found");
    assert_eq!(token_b, token_b_by_symbol);

    let db_nft_token = TokensSchema(&mut storage)
        .get_token(TokenLike::Id(nft.id))
        .await
        .expect("Get nft failed")
        .expect("Token not found");
    assert_eq!(db_nft_token, nft);
    Ok(())
}

/// Checks the store/load routine for `ticker_price` table.
#[db_test]
async fn test_ticker_price(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    const TOKEN_ID: TokenId = TokenId(0);
    // No entry exists yet.
    let loaded = storage
        .tokens_schema()
        .get_historical_ticker_price(TOKEN_ID)
        .await?;
    assert!(loaded.is_none());
    // Store new price.
    // `usd_price` is not a finite decimal, so we expect it to be rounded
    // up to `STORED_USD_PRICE_PRECISION` digits.
    let price = TokenPrice {
        usd_price: Ratio::new(BigUint::from(4u32), BigUint::from(9u32)),
        last_updated: chrono::Utc::now(),
    };

    storage
        .tokens_schema()
        .update_historical_ticker_price(TOKEN_ID, price.clone())
        .await?;
    // Load it again.
    let loaded = storage
        .tokens_schema()
        .get_historical_ticker_price(TOKEN_ID)
        .await?
        .expect("couldn't load token price");

    // During the load the price was converted back to ratio.
    let expected_stored_decimal =
        ratio_to_big_decimal(&price.usd_price, STORED_USD_PRICE_PRECISION);
    let expected_price = big_decimal_to_ratio(&expected_stored_decimal).unwrap();

    assert_eq!(loaded.usd_price, expected_price);
    // Comparing this fields directly might fail, use timestamps.
    assert_eq!(
        loaded.last_updated.timestamp(),
        price.last_updated.timestamp()
    );

    Ok(())
}

/// Checks the store/load routine for `ticker_market_volume` table and load tokens by market volume.
#[db_test]
async fn test_market_volume(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    const TOKEN_ID: TokenId = TokenId(0);

    let market_volume = TokenMarketVolume {
        market_volume: Ratio::new(BigUint::from(2u32), BigUint::from(5u32)),
        last_updated: chrono::Utc::now(),
    };

    storage
        .tokens_schema()
        .update_token_market_volume(TOKEN_ID, market_volume.clone())
        .await?;

    let loaded = storage
        .tokens_schema()
        .get_token_market_volume(TOKEN_ID)
        .await?
        .expect("couldn't load market volume");

    assert_eq!(loaded.market_volume, market_volume.market_volume);

    assert_eq!(
        loaded.last_updated.timestamp(),
        market_volume.last_updated.timestamp()
    );

    let tokens = TokensSchema(&mut storage)
        .load_tokens_by_market_volume(Ratio::new(BigUint::from(3u32), BigUint::from(5u32)))
        .await
        .expect("Load tokens by market volume query failed");
    assert_eq!(tokens.len(), 0);

    let tokens = TokensSchema(&mut storage)
        .load_tokens_by_market_volume(Ratio::new(BigUint::from(2u32), BigUint::from(5u32)))
        .await
        .expect("Load tokens by market volume query failed");
    assert_eq!(tokens.len(), 1);

    Ok(())
}

/// Checks the store/load factories for nft
#[db_test]
async fn test_nfts_with_factories(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let token_id = TokenId(2u32.pow(16) + 10);
    let creator_account_id = 5;
    let symbol = String::from("SYMBOL");
    let diff = StorageAccountDiff::MintNFT(StorageMintNFTUpdate {
        token_id: *token_id as i32,
        serial_id: 0,
        creator_account_id,
        creator_address: Address::default().as_bytes().to_vec(),
        address: Address::from_str("2222222222222222222222222222222222222222")
            .unwrap()
            .as_bytes()
            .to_vec(),
        content_hash: H256::default().as_bytes().to_vec(),
        update_order_id: 0,
        block_number: 0,
        symbol: symbol.clone(),
    });
    storage
        .chain()
        .state_schema()
        .apply_storage_account_diff(diff)
        .await?;

    let zksync_account = ZkSyncAccount::rand();
    zksync_account.set_account_id(Some(AccountId(123)));
    let op = ZkSyncOp::WithdrawNFT(Box::new(WithdrawNFTOp {
        tx: zksync_account
            .sign_withdraw_nft(
                token_id,
                TokenId(0),
                &symbol,
                Default::default(),
                &Default::default(),
                None,
                false,
                Default::default(),
            )
            .0,
        creator_id: Default::default(),
        creator_address: Default::default(),
        serial_id: Default::default(),
        content_hash: Default::default(),
    }));

    let executed_tx = ExecutedTx {
        signed_tx: op.try_get_tx().unwrap().into(),
        success: true,
        op: Some(op),
        fail_reason: None,
        block_index: Some(0),
        created_at: Utc::now(),
        batch_id: None,
    };
    let executed_op = ExecutedOperations::Tx(Box::new(executed_tx));
    let block_number = BlockNumber(1);
    storage
        .chain()
        .block_schema()
        .save_block_transactions(block_number, vec![executed_op])
        .await?;

    let default_factory_address =
        Address::from_str("1111111111111111111111111111111111111111").unwrap();
    storage
        .config_schema()
        .store_config(
            Default::default(),
            Default::default(),
            default_factory_address,
        )
        .await?;

    let nft = storage
        .tokens_schema()
        .get_nft_with_factories(token_id)
        .await?
        .unwrap();
    assert_eq!(nft.symbol, symbol);
    assert_eq!(nft.current_factory, default_factory_address);
    assert!(nft.withdrawn_factory.is_none());

    storage
        .chain()
        .block_schema()
        .store_factories_for_block_withdraw_nfts(block_number, block_number)
        .await?;

    let nft = storage
        .tokens_schema()
        .get_nft_with_factories(token_id)
        .await?
        .unwrap();
    assert_eq!(nft.current_factory, default_factory_address);
    assert_eq!(nft.withdrawn_factory.unwrap(), default_factory_address);

    let new_factory_address =
        Address::from_str("51f610535ab3c695e0bcef6b7827f8d4a3472f01").unwrap();
    storage
        .tokens_schema()
        .store_nft_factory(
            AccountId(creator_account_id as u32),
            Default::default(),
            new_factory_address,
        )
        .await?;

    let nft = storage
        .tokens_schema()
        .get_nft_with_factories(token_id)
        .await?
        .unwrap();
    assert_eq!(nft.current_factory, new_factory_address);
    assert_eq!(nft.withdrawn_factory.unwrap(), default_factory_address);

    Ok(())
}
