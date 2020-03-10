// External imports
// Workspace imports
// Local imports
use crate::tests::db_test;
use crate::{
    tokens::{records::Token, TokensSchema},
    StorageProcessor,
};

/// Verifies the token save & load mechanism.
#[test]
fn tokens_storage() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        // There should be only Ethereum main token by default.
        let tokens = TokensSchema(&conn)
            .load_tokens()
            .expect("Load tokens query failed");
        assert_eq!(tokens.len(), 1);
        assert_eq!(
            tokens[&0],
            Token {
                id: 0,
                address: "0000000000000000000000000000000000000000".into(),
                symbol: "ETH".into(),
            }
        );

        // Add two tokens.
        let token_a_id = 1;
        let token_a_symbol = "ABC";
        let token_a_addr = "0000000000000000000000000000000000000001";
        let token_b_id = 2;
        let token_b_symbol = "DEF";
        let token_b_addr = "0000000000000000000000000000000000000002";

        TokensSchema(&conn)
            .store_token(token_a_id, token_a_addr, token_a_symbol)
            .expect("Store tokens query failed");
        TokensSchema(&conn)
            .store_token(token_b_id, token_b_addr, token_b_symbol)
            .expect("Store tokens query failed");

        // Load tokens again.
        let tokens = TokensSchema(&conn)
            .load_tokens()
            .expect("Load tokens query failed");

        assert_eq!(tokens.len(), 3);
        assert_eq!(
            tokens[&token_a_id],
            Token {
                id: token_a_id as i32,
                address: token_a_addr.into(),
                symbol: token_a_symbol.into(),
            }
        );
        assert_eq!(
            tokens[&token_b_id],
            Token {
                id: token_b_id as i32,
                address: token_b_addr.into(),
                symbol: token_b_symbol.into(),
            }
        );

        Ok(())
    });
}
