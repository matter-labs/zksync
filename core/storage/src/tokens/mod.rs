// Built-in deps
use std::collections::HashMap;
// External imports
use diesel::prelude::*;
// Workspace imports
use models::node::{Address, Token, TokenId, TokenLike};
// Local imports
use self::records::DbToken;
use crate::schema::*;
use crate::StorageProcessor;

pub mod records;

/// Tokens schema handles the `tokens` table, providing methods to
/// get and store new tokens.
#[derive(Debug)]
pub struct TokensSchema<'a>(pub &'a StorageProcessor);

pub(self) fn address_to_stored_string(address: &Address) -> String {
    format!("0x{:x}", address)
}

pub(self) fn stored_str_address_to_address(address: &str) -> Address {
    address[2..]
        .parse()
        .expect("failed to parse stored db address")
}

impl<'a> TokensSchema<'a> {
    /// Persists the token in the database.
    pub fn store_token(&self, token: Token) -> QueryResult<()> {
        let new_token: DbToken = token.into();
        diesel::insert_into(tokens::table)
            .values(&new_token)
            .on_conflict(tokens::id)
            .do_update()
            // update token address but not symbol -- so we can update it externally
            .set(tokens::address.eq(new_token.address.clone()))
            .execute(self.0.conn())
            .map(drop)
    }

    /// Loads all the stored tokens from the database.
    /// Alongside with the tokens added via `store_token` method, the default `ETH` token
    /// is returned.
    pub fn load_tokens(&self) -> QueryResult<HashMap<TokenId, Token>> {
        let tokens = tokens::table
            .order(tokens::id.asc())
            .load::<DbToken>(self.0.conn())?;
        Ok(tokens
            .into_iter()
            .map(|t| {
                let token: Token = t.into();
                (token.id, token)
            })
            .collect())
    }

    /// Given the numeric token ID, symbol or address, returns token.
    pub fn get_token(&self, token_like: TokenLike) -> QueryResult<Option<Token>> {
        let db_token = match token_like {
            TokenLike::Id(token_id) => tokens::table
                .find(i32::from(token_id))
                .first::<DbToken>(self.0.conn())
                .optional(),
            TokenLike::Address(token_address) => tokens::table
                .filter(tokens::address.eq(address_to_stored_string(&token_address)))
                .first::<DbToken>(self.0.conn())
                .optional(),
            TokenLike::Symbol(token_symbol) => tokens::table
                .filter(tokens::symbol.eq(token_symbol))
                .first::<DbToken>(self.0.conn())
                .optional(),
        }?;
        Ok(db_token.map(|t| t.into()))
    }
}

#[cfg(test)]
pub mod test {
    use crate::tokens::{address_to_stored_string, stored_str_address_to_address};
    use models::node::Address;

    #[test]
    fn address_store_roundtrip() {
        let address = Address::random();
        let stored_address = address_to_stored_string(&address);
        assert_eq!(address, stored_str_address_to_address(&stored_address));
    }
}
