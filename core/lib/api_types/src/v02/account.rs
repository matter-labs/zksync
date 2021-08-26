use std::collections::{BTreeMap, HashMap};

use num::{BigUint, ToPrimitive};
use serde::{Deserialize, Serialize};

use zksync_types::{
    AccountId, Address, BlockNumber, Nonce, PriorityOp, PubKeyHash, TokenId, ZkSyncPriorityOp,
};
use zksync_utils::{BigUintSerdeAsRadix10Str, BigUintSerdeWrapper};

use super::token::NFT;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountState {
    pub depositing: DepositingAccountBalances,
    pub committed: Option<Account>,
    pub finalized: Option<Account>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub account_id: AccountId,
    pub address: Address,
    pub nonce: Nonce,
    pub pub_key_hash: PubKeyHash,
    pub last_update_in_block: BlockNumber,
    pub balances: BTreeMap<String, BigUintSerdeWrapper>,
    pub nfts: BTreeMap<TokenId, NFT>,
    pub minted_nfts: BTreeMap<TokenId, NFT>,
    pub account_type: Option<EthAccountType>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub enum AccountAddressOrId {
    Address(Address),
    Id(AccountId),
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum EthAccountType {
    Owned,
    CREATE2,
    No2FA,
}

/// Information about ongoing deposits for certain recipient address.
///
/// Please note that since this response is based on the events that are
/// currently awaiting confirmations, this information is approximate:
/// blocks on Ethereum can be reverted, and final list of executed deposits
/// can differ from this estimation.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OngoingDepositsResp {
    pub deposits: Vec<OngoingDeposit>,
}

/// Flattened `PriorityOp` object representing a deposit operation.
/// Used in the `OngoingDepositsResp`.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OngoingDeposit {
    pub received_on_block: u64,
    pub token_id: TokenId,
    pub amount: u128,
}

impl OngoingDeposit {
    pub fn new(priority_op: PriorityOp) -> Self {
        let (token_id, amount) = match priority_op.data {
            ZkSyncPriorityOp::Deposit(deposit) => (
                deposit.token,
                deposit
                    .amount
                    .to_u128()
                    .expect("Deposit amount should be less then u128::max()"),
            ),
            other => {
                panic!("Incorrect input for OngoingDeposit: {:?}", other);
            }
        };

        Self {
            received_on_block: priority_op.eth_block,
            token_id,
            amount,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DepositingFunds {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub amount: BigUint,
    pub expected_accept_block: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DepositingAccountBalances {
    pub balances: HashMap<String, DepositingFunds>,
}
