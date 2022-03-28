// External imports
use serde::{Deserialize, Serialize};
// Workspace imports
// Local imports

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountTreeCache {
    pub block: i64,
    pub tree_cache: String,
}
