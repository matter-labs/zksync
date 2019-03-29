#[macro_use]
extern crate serde_derive;
extern crate serde_bytes;
extern crate plasma;

use plasma::models::*;
use std::sync::mpsc::{Sender};

#[derive(Debug, Serialize, Deserialize)]
pub struct TransferTxConfirmation {
    pub block_number:   BlockNumber,
    pub signature:      String,
}

pub type TransferTxResult = Result<TransferTxConfirmation, TransferApplicationError>;

#[derive(Debug, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub next_block_at_max: Option<u64>,
}

pub enum StateKeeperRequest{
    AddTransferTx(TransferTx, Sender<TransferTxResult>),
    AddBlock(Block),
    GetAccount(u32, Sender<Option<Account>>),
    GetNetworkStatus(Sender<NetworkStatus>),
    TimerTick,
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

        #[serde(with = "serde_bytes")]
        public_data:    Vec<u8>,
    },
}

pub struct ProverRequest(pub BlockNumber, pub Block, pub EthBlockData, pub AccountMap);

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
