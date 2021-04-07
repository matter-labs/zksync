use crate::{
    tx::{EthBatchSignatures, TxEthSignature},
    ZkSyncTx,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IncomingTxBatch {
    pub txs: Vec<ZkSyncTx>,
    pub signature: EthBatchSignatures,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct IncomingTx {
    pub tx: ZkSyncTx,
    pub signature: Option<TxEthSignature>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum L1Status {
    //Pending,
    Committed,
    Finalized,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum L2Status {
    Queued,
    Committed,
    Finalized,
    Rejected,
}

impl From<L1Status> for L2Status {
    fn from(status: L1Status) -> Self {
        match status {
            L1Status::Committed => L2Status::Committed,
            L1Status::Finalized => L2Status::Finalized,
        }
    }
}
