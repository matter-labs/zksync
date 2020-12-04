// External imports
use num::{rational::Ratio, BigUint};
// Workspace imports
use zksync_types::{Token, TokenId, TokenLike, TokenPrice};
use zksync_utils::{big_decimal_to_ratio, ratio_to_big_decimal};
// Local imports
use crate::tests::db_test;
use crate::{
    tokens::{TokensSchema, STORED_USD_PRICE_PRECISION},
    QueryResult, StorageProcessor,
};

/// Verifies the token save & load mechanism.
#[db_test]
async fn tokens_storage(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // There should be only Ethereum main token by default.
    assert_eq!(storage.tokens_schema().get_count().await?, 1);
    let tokens = TokensSchema(&mut storage)
        .load_tokens()
        .await
        .expect("Load tokens query failed");
    assert_eq!(tokens.len(), 1);
    let eth_token = Token {
        id: 0,
        address: "0000000000000000000000000000000000000000".parse().unwrap(),
        symbol: "ETH".into(),
        decimals: 18,
    };
    assert_eq!(tokens[&0], eth_token);

    // Add two tokens.
    let token_a = Token {
        id: 1,
        address: "0000000000000000000000000000000000000001".parse().unwrap(),
        symbol: "ABC".into(),
        decimals: 9,
    };
    let token_b = Token {
        id: 2,
        address: "0000000000000000000000000000000000000002".parse().unwrap(),
        symbol: "DEF".into(),
        decimals: 6,
    };

    TokensSchema(&mut storage)
        .store_token(token_a.clone())
        .await
        .expect("Store tokens query failed");
    TokensSchema(&mut storage)
        .store_token(token_b.clone())
        .await
        .expect("Store tokens query failed");
    // The count is updated.
    assert_eq!(storage.tokens_schema().get_count().await?, 3);

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

    // Now check that storing the token that already exists is the same as updating it.
    let token_c = Token {
        id: 2,
        address: "0000000000000000000000000000000000000008".parse().unwrap(),
        symbol: "BAT".into(),
        decimals: 6,
    };
    TokensSchema(&mut storage)
        .store_token(token_c.clone())
        .await
        .expect("Store tokens query failed");
    // Load updated token.
    let token_c_by_id = TokensSchema(&mut storage)
        .get_token(TokenLike::Id(token_c.id))
        .await
        .expect("get token query failed")
        .expect("token by id not found");
    assert_eq!(token_c, token_c_by_id);

    Ok(())
}

/// Checks the store/load routine for `ticker_price` table.
#[db_test]
async fn test_ticker_price(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    const TOKEN_ID: TokenId = 0;
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
