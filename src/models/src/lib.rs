#[macro_use]
extern crate serde_derive;

use serde_bytes;
#[macro_use]
extern crate lazy_static;

pub mod config;
pub mod encoder;

use plasma::models::*;
use std::sync::mpsc::Sender;

pub use eth_client::TxMeta;

#[derive(Debug, Serialize, Deserialize)]
pub struct TransferTxConfirmation {
    pub block_number: BlockNumber,
    pub signature: String,
}

pub type TransferTxResult = Result<TransferTxConfirmation, TransferApplicationError>;

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct NetworkStatus {
    pub next_block_at_max: Option<u64>,
    pub last_committed: BlockNumber,
    pub last_verified: BlockNumber,
    pub total_transactions: u32,
    pub outstanding_txs: u32,
}

pub type EncodedProof = [U256; 8];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EthBlockData {
    Transfer {
        total_fees: U128,

        #[serde(with = "serde_bytes")]
        public_data: Vec<u8>,
    },
    Deposit {
        batch_number: BatchNumber,
    },
    Exit {
        batch_number: BatchNumber,

        #[serde(with = "serde_bytes")]
        public_data: Vec<u8>,
    },
}

pub struct ProverRequest(pub BlockNumber);

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Action {
    Commit,
    Verify { proof: EncodedProof },
}

impl std::string::ToString for Action {
    fn to_string(&self) -> String {
        match self {
            Action::Commit => "Commit".to_owned(),
            Action::Verify { proof: _ } => "Verify".to_owned(),
        }
    }
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.to_string())
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Operation {
    pub id: Option<i32>,
    pub action: Action,
    pub block: Block,
    pub accounts_updated: Option<AccountMap>,

    #[serde(skip)]
    pub tx_meta: Option<TxMeta>,
}

pub enum ProtoBlock {
    Transfer,
    Deposit(BatchNumber, Vec<DepositTx>),
    Exit(BatchNumber, Vec<ExitTx>),
}

pub enum StateKeeperRequest {
    AddTransferTx(TransferTx, Sender<TransferTxResult>),
    AddBlock(ProtoBlock),
    GetAccount(u32, Sender<Option<Account>>),
    GetNetworkStatus(Sender<NetworkStatus>),
    TimerTick,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitRequest {
    pub block: Block,
    pub accounts_updated: AccountMap,
}

pub const ACTION_COMMIT: &'static str = "Commit";
pub const ACTION_VERIFY: &'static str = "Verify";

pub enum ActionType {
    COMMIT,
    VERIFY,
}

impl std::string::ToString for ActionType {
    fn to_string(&self) -> String {
        match self {
            ActionType::COMMIT => ACTION_COMMIT.to_owned(),
            ActionType::VERIFY => ACTION_VERIFY.to_owned(),
        }
    }
}
