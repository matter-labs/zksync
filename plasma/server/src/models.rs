use web3::types::{U256, U128, H256};
use plasma::models::{BatchNumber, AccountMap, Block, PublicKey};
use std::sync::mpsc::{Sender};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EthOperation {
    Commit{
        block_number:       u32, 
        new_root:           H256, 
        block_data:         EthBlockData,
        accounts_updated:   AccountMap,
    },
    Verify{
        block_number:       u32, 
        proof:              EncodedProof, 
        block_data:         EthBlockData,
        accounts_updated:   AccountMap,
    },
    StartDepositBatch,
    StartExitBatch,
    // ...
}

#[derive(Queryable)]
pub struct StoredOperation {
    pub id:         i32,
    pub data:       serde_json::Value,
    pub addr:       String,
    pub nonce:      i32,
    pub created_at: std::time::SystemTime,
}