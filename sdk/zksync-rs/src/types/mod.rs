use num::BigUint;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use zksync_types::{AccountId, Address, Nonce, PubKeyHash, Token};
use zksync_utils::{BigUintSerdeAsRadix10Str, BigUintSerdeWrapper};

pub mod network;

pub type Tokens = HashMap<String, Token>;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountState {
    pub balances: HashMap<String, BigUintSerdeWrapper>,
    pub nonce: Nonce,
    pub pub_key_hash: PubKeyHash,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DepositingFunds {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    amount: BigUint,
    expected_accept_block: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DepositingAccountBalances {
    balances: HashMap<String, DepositingFunds>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BlockStatus {
    Committed,
    Verified,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub address: Address,
    pub id: Option<AccountId>,
    pub depositing: DepositingAccountBalances,
    pub committed: AccountState,
    pub verified: AccountState,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockInfo {
    pub block_number: i64,
    pub committed: bool,
    pub verified: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TransactionInfo {
    pub executed: bool,
    pub success: Option<bool>,
    pub fail_reason: Option<String>,
    pub block: Option<BlockInfo>,
}

impl TransactionInfo {
    /// Indicates whether this transaction is verified.
    pub fn is_verified(&self) -> bool {
        self.executed && self.block.as_ref().filter(|x| x.verified).is_some()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EthOpInfo {
    pub executed: bool,
    pub block: Option<BlockInfo>,
}

impl EthOpInfo {
    /// Indicates whether this operation is verified.
    pub fn is_verified(&self) -> bool {
        self.executed && self.block.as_ref().filter(|x| x.verified).is_some()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContractAddress {
    pub main_contract: String,
    pub gov_contract: String,
}

/// Flattened `PriorityOp` object representing a deposit operation.
/// Used in the `OngoingDepositsResp`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OngoingDeposit {
    pub received_on_block: u64,
    pub token_id: u16,
    pub amount: u128,
    pub eth_tx_hash: String,
}

/// Information about ongoing deposits for certain recipient address.
///
/// Please note that since this response is based on the events that are
/// currently awaiting confirmations, this information is approximate:
/// blocks on Ethereum can be reverted, and final list of executed deposits
/// can differ from the this estimation.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OngoingDeposits {
    /// Address for which response is served.
    pub address: Address,
    /// List of tuples (Eth block number, Deposit operation) of ongoing
    /// deposit operations.
    pub deposits: Vec<OngoingDeposit>,

    /// Amount of confirmations required for every deposit to be processed.
    pub confirmations_for_eth_event: u64,

    /// Estimated block number for deposits completions:
    /// all the deposit operations for provided address are expected to be
    /// accepted in the zkSync network upon reaching this blocks.
    ///
    /// Can be `None` if there are no ongoing deposits.
    pub estimated_deposits_approval_block: Option<u64>,
}

/// Type of the fee calculation pattern.
/// Unlike the `TxFeeTypes`, this enum represents the fee
/// from the point of zkSync view, rather than from the users
/// point of view.
/// Users do not divide transfers into `Transfer` and
/// `TransferToNew`, while in zkSync it's two different operations.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum OutputFeeType {
    Transfer,
    TransferToNew,
    FastWithdraw,
    Withdraw,
    ChangePubKey { onchain_pubkey_auth: bool },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Fee {
    pub fee_type: OutputFeeType,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub gas_tx_amount: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub gas_price_wei: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub gas_fee: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub zkp_fee: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub total_fee: BigUint,
}
