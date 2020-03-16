// External imports
// Workspace imports
// Local imports
use crate::schema::*;

#[derive(Debug, Queryable, QueryableByName)]
#[table_name = "server_config"]
pub struct ServerConfig {
    pub id: bool,
    pub contract_addr: Option<String>,
    pub gov_contract_addr: Option<String>,
}
