// External imports
// Workspace imports
// Local imports
use crate::tests::db_test;
use crate::{tokens::TokensSchema, QueryResult, StorageProcessor};
use zksync_types::{Token, TokenLike};

/// Verifies the token save & load mechanism.
#[db_test]
async fn tokens_storage(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // There should be only Ethereum main token by default.
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

    Ok(())
}
