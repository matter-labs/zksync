use std::collections::HashMap;

// External uses
use jsonrpc_core::{Error, Result};
use num::{BigUint, ToPrimitive};
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_storage::StorageProcessor;
use zksync_types::{
    tx::TxEthSignatureVariant, Account, AccountId, Address, Nonce, PriorityOp, PubKeyHash, TokenId,
    ZkSyncPriorityOp, ZkSyncTx,
};
use zksync_utils::{BigUintSerdeAsRadix10Str, BigUintSerdeWrapper};

// This wrong dependency, but the whole data about account info stored in this place
use zksync_api_client::rest::v1::accounts::NFT;

// Local uses
use crate::{
    api_server::v1::accounts::account_state_from_storage, utils::token_db_cache::TokenDBCache,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxWithSignature {
    pub tx: ZkSyncTx,
    #[serde(default)]
    pub signature: TxEthSignatureVariant,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResponseAccountState {
    pub balances: HashMap<String, BigUintSerdeWrapper>,
    pub nfts: HashMap<TokenId, NFT>,
    pub minted_nfts: HashMap<TokenId, NFT>,
    pub nonce: Nonce,
    pub pub_key_hash: PubKeyHash,
}

impl ResponseAccountState {
    pub async fn try_restore(
        storage: &mut StorageProcessor<'_>,
        tokens: &TokenDBCache,
        account: Account,
    ) -> Result<Self> {
        let inner = account_state_from_storage(storage, tokens, &account)
            .await
            .map_err(|_| Error::internal_error())?;

        // Old code used `HashMap` as well and didn't rely on the particular order,
        // so here we use `HashMap` as well for the consistency.
        let balances: HashMap<_, _> = inner.balances.into_iter().collect();

        Ok(Self {
            balances,
            nfts: inner.nfts,
            minted_nfts: inner.minted_nfts,
            nonce: inner.nonce,
            pub_key_hash: inner.pub_key_hash,
        })
    }
}

#[derive(Debug, Clone)]
pub struct AccountStateInfo {
    pub account_id: Option<AccountId>,
    pub committed: ResponseAccountState,
    pub verified: ResponseAccountState,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DepositingFunds {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub amount: BigUint,
    pub expected_accept_block: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DepositingAccountBalances {
    pub balances: HashMap<String, DepositingFunds>,
}

impl DepositingAccountBalances {
    pub async fn from_pending_ops(
        storage: &mut StorageProcessor<'_>,
        tokens: &TokenDBCache,
        pending_ops: OngoingDepositsResp,
    ) -> Result<Self> {
        let mut balances = HashMap::new();

        for op in pending_ops.deposits {
            let token_symbol = if *op.token_id == 0 {
                "ETH".to_string()
            } else {
                tokens
                    .get_token(storage, op.token_id)
                    .await
                    .map_err(|_| Error::internal_error())?
                    .ok_or_else(Error::internal_error)?
                    .symbol
            };

            let expected_accept_block =
                op.received_on_block + pending_ops.confirmations_for_eth_event;

            let balance = balances
                .entry(token_symbol)
                .or_insert_with(DepositingFunds::default);

            balance.amount += BigUint::from(op.amount);

            // `balance.expected_accept_block` should be the greatest block number among
            // all the deposits for a certain token.
            if expected_accept_block > balance.expected_accept_block {
                balance.expected_accept_block = expected_accept_block;
            }
        }

        Ok(Self { balances })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfoResp {
    pub address: Address,
    pub id: Option<AccountId>,
    pub depositing: DepositingAccountBalances,
    pub committed: ResponseAccountState,
    pub verified: ResponseAccountState,
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
pub struct TransactionInfoResp {
    pub executed: bool,
    pub success: Option<bool>,
    pub fail_reason: Option<String>,
    pub block: Option<BlockInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ETHOpInfoResp {
    pub executed: bool,
    pub block: Option<BlockInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContractAddressResp {
    pub main_contract: String,
    pub gov_contract: String,
}

/// Flattened `PriorityOp` object representing a deposit operation.
/// Used in the `OngoingDepositsResp`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OngoingDeposit {
    pub received_on_block: u64,
    pub token_id: TokenId,
    pub amount: u128,
    pub eth_tx_hash: String,
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

        let eth_tx_hash = hex::encode(&priority_op.eth_hash);

        Self {
            received_on_block: priority_op.eth_block,
            token_id,
            amount,
            eth_tx_hash,
        }
    }
}

/// Information about ongoing deposits for certain recipient address.
///
/// Please note that since this response is based on the events that are
/// currently awaiting confirmations, this information is approximate:
/// blocks on Ethereum can be reverted, and final list of executed deposits
/// can differ from the this estimation.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OngoingDepositsResp {
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
