use web3::types::{U256, U128, H256};
use plasma::models::{BatchNumber, AccountMap, Block, PublicKey};
use std::sync::mpsc::{Sender};
use crate::schema::*;

// MemPool will provide a channel to return result of block processing
// In case of error, block is returned with invalid transactions removed
pub enum StateProcessingRequest{
    ApplyBlock(Block, Option<Sender<Result<(),Block>>>), // return result, sending block back
    GetPubKey(u32, Sender<Option<PublicKey>>),   // return public key if found
}

pub type EncodedProof = [U256; 8];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EthBlockData {
    Transfer{
        total_fees:     U128,

        #[serde(with = "serde_bytes")]
        public_data:    Vec<u8>,
    },
    Deposit{
        batch_number:   BatchNumber,
    },
    Exit{
        batch_number:   BatchNumber,
    },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Action {
    Commit{
        new_root:   H256,
        block:      Option<Block>,
    },
    Verify{
        proof:      EncodedProof, 
    },
}

impl std::string::ToString for Action {
    fn to_string(&self) -> String {
        match self {
            Action::Commit{new_root: _, block: _}   => "Commit".to_owned(),
            Action::Verify{proof: _}                => "Verify".to_owned(),
        }
    }
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub action:             Action,
    pub block_number:       u32, 
    pub block_data:         EthBlockData,
    pub accounts_updated:   AccountMap,
}

#[derive(Debug, Queryable, QueryableByName)]
#[table_name="operations"]
pub struct StoredOperation {
    pub id:             i32,
    pub data:           serde_json::Value,
    pub addr:           String,
    pub nonce:          i32,
    pub block_number:   i32,
    pub action_type:    String,
    pub created_at:     std::time::SystemTime,
}