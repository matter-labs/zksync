// External imports
// Workspace imports
// Local imports
use crate::tests::db_test;
use crate::{tokens::TokensSchema, StorageProcessor};
use models::node::{Token, TokenLike};

/// Verifies the token save & load mechanism.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn tokens_storage() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        // There should be only Ethereum main token by default.
        let tokens = TokensSchema(&conn)
            .load_tokens()
            .expect("Load tokens query failed");
        assert_eq!(tokens.len(), 1);
        let eth_token = Token {
            id: 0,
            address: "0000000000000000000000000000000000000000".parse().unwrap(),
            symbol: "ETH".into(),
            precision: 18,
        };
        assert_eq!(tokens[&0], eth_token);

        // Add two tokens.
        let token_a = Token {
            id: 1,
            address: "0000000000000000000000000000000000000001".parse().unwrap(),
            symbol: "ABC".into(),
            precision: 9,
        };
        let token_b = Token {
            id: 2,
            address: "0000000000000000000000000000000000000002".parse().unwrap(),
            symbol: "DEF".into(),
            precision: 6,
        };

        TokensSchema(&conn)
            .store_token(token_a.clone())
            .expect("Store tokens query failed");
        TokensSchema(&conn)
            .store_token(token_b.clone())
            .expect("Store tokens query failed");

        // Load tokens again.
        let tokens = TokensSchema(&conn)
            .load_tokens()
            .expect("Load tokens query failed");

        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[&eth_token.id], eth_token);
        assert_eq!(tokens[&token_a.id], token_a);
        assert_eq!(tokens[&token_b.id], token_b);

        let token_b_by_id = TokensSchema(&conn)
            .get_token(TokenLike::Id(token_b.id))
            .expect("get token query failed")
            .expect("token by id not found");
        assert_eq!(token_b, token_b_by_id);

        let token_b_by_address = TokensSchema(&conn)
            .get_token(TokenLike::Address(token_b.address))
            .expect("get token query failed")
            .expect("token by address not found");
        assert_eq!(token_b, token_b_by_address);

        let token_b_by_symbol = TokensSchema(&conn)
            .get_token(TokenLike::Symbol(token_b.symbol.clone()))
            .expect("get token query failed")
            .expect("token by symbol not found");
        assert_eq!(token_b, token_b_by_symbol);

        Ok(())
    });
}
