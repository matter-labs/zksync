// External imports
use sqlx::FromRow;
// Workspace imports
// Local imports

#[derive(Debug, FromRow)]
pub struct ServerConfig {
    pub id: bool,
    pub contract_addr: Option<String>,
    pub gov_contract_addr: Option<String>,
}
