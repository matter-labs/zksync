// Built-in deps
use std::collections::HashMap;
// External imports
use diesel::prelude::*;
// Workspace imports
use models::node::TokenId;
// Local imports
use self::records::Token;
use crate::schema::*;
use crate::StorageProcessor;

pub mod records;

pub struct TokensSchema<'a>(pub &'a StorageProcessor);

impl<'a> TokensSchema<'a> {
    pub fn store_token(&self, id: TokenId, address: &str, symbol: &str) -> QueryResult<()> {
        let new_token = Token {
            id: i32::from(id),
            address: address.to_string(),
            symbol: symbol.to_string(),
        };
        diesel::insert_into(tokens::table)
            .values(&new_token)
            .on_conflict(tokens::id)
            .do_update()
            // update token address but not symbol -- so we can update it externally
            .set(tokens::address.eq(new_token.address.clone()))
            .execute(self.0.conn())
            .map(drop)
    }

    pub fn load_tokens(&self) -> QueryResult<HashMap<TokenId, Token>> {
        let tokens = tokens::table
            .order(tokens::id.asc())
            .load::<Token>(self.0.conn())?;
        Ok(tokens.into_iter().map(|t| (t.id as TokenId, t)).collect())
    }
}
