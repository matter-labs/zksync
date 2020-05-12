// External imports
use serde_derive::{Deserialize, Serialize};
// Workspace imports
// Local imports
use crate::schema::*;
use crate::tokens::utils::{address_to_stored_string, stored_str_address_to_address};
use models::node::{Token, TokenId};

#[derive(
    Debug,
    Clone,
    Insertable,
    QueryableByName,
    Queryable,
    Serialize,
    Deserialize,
    AsChangeset,
    PartialEq,
)]
#[table_name = "tokens"]

pub struct DbToken {
    pub id: i32,
    pub address: String,
    pub symbol: String,
    pub precision: i32,
}

impl From<Token> for DbToken {
    fn from(token: Token) -> Self {
        Self {
            id: token.id as i32,
            address: address_to_stored_string(&token.address),
            symbol: token.symbol,
            precision: token.precision as i32,
        }
    }
}

impl Into<Token> for DbToken {
    fn into(self) -> Token {
        Token {
            id: self.id as TokenId,
            address: stored_str_address_to_address(&self.address),
            symbol: self.symbol,
            precision: self.precision as u8,
        }
    }
}
