// External imports
use serde_derive::{Deserialize, Serialize};
// Workspace imports
// Local imports
use crate::schema::*;

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
pub struct Token {
    pub id: i32,
    pub address: String,
    pub symbol: String,
}
