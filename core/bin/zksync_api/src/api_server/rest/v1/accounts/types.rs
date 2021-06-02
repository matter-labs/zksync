//! Data transfer objects used in the accounts API implementation

// Built-in uses
use std::collections::BTreeMap;

// Workspace uses
pub use zksync_api_client::rest::v1::accounts::{
    AccountInfo, AccountOpReceipt, AccountQuery, AccountReceipts, AccountReceiptsQuery,
    AccountState, AccountTxReceipt, DepositingBalances, DepositingFunds, PendingAccountOpReceipt,
    SearchDirection, TxLocation,
};
use zksync_storage::{
    chain::operations_ext::{
        records::{AccountOpReceiptResponse, AccountTxReceiptResponse},
        SearchDirection as StorageSearchDirection,
    },
    QueryResult, StorageProcessor,
};
use zksync_types::{tx::TxHash, Account, BlockNumber, PriorityOp, ZkSyncPriorityOp, H256};

// Local uses
use crate::{api_server::v1::MAX_LIMIT, utils::token_db_cache::TokenDBCache};

use super::{
    super::{transactions::Receipt, ApiError},
    unable_to_find_token,
};

pub(super) mod convert {
    use std::collections::HashMap;
    use zksync_crypto::params::{MIN_NFT_TOKEN_ID, NFT_TOKEN_ID_VAL};

    use super::*;

    pub async fn account_state_from_storage(
        storage: &mut StorageProcessor<'_>,
        tokens: &TokenDBCache,
        account: &Account,
    ) -> QueryResult<AccountState> {
        let mut balances = BTreeMap::new();
        let mut nfts = HashMap::new();
        for (token_id, balance) in account.get_nonzero_balances() {
            match token_id.0 {
                NFT_TOKEN_ID_VAL => {
                    // Don't include special token to balances or nfts
                }
                MIN_NFT_TOKEN_ID..=NFT_TOKEN_ID_VAL => {
                    // https://github.com/rust-lang/rust/issues/37854
                    // Exclusive range is an experimental feature, but we have already checked the last value in the previous step
                    nfts.insert(
                        token_id,
                        tokens
                            .get_nft_by_id(storage, token_id)
                            .await?
                            .ok_or_else(|| unable_to_find_token(token_id))?
                            .into(),
                    );
                }
                _ => {
                    let token_symbol = tokens
                        .token_symbol(storage, token_id)
                        .await?
                        .ok_or_else(|| unable_to_find_token(token_id))?;
                    balances.insert(token_symbol, balance);
                }
            }
        }
        let minted_nfts = account
            .minted_nfts
            .iter()
            .map(|(id, nft)| (*id, nft.clone().into()))
            .collect();

        Ok(AccountState {
            balances,
            nfts,
            minted_nfts,
            nonce: account.nonce,
            pub_key_hash: account.pub_key_hash,
        })
    }

    pub fn search_direction_as_storage(direction: SearchDirection) -> StorageSearchDirection {
        match direction {
            SearchDirection::Older => StorageSearchDirection::Older,
            SearchDirection::Newer => StorageSearchDirection::Newer,
        }
    }

    pub async fn depositing_balances_from_pending_ops(
        storage: &mut StorageProcessor<'_>,
        tokens: &TokenDBCache,
        ongoing_ops: Vec<PriorityOp>,
        confirmations_for_eth_event: BlockNumber,
    ) -> QueryResult<DepositingBalances> {
        let mut balances = BTreeMap::new();

        for op in ongoing_ops {
            let received_on_block = op.eth_block;
            let (amount, token_id) = match op.data {
                ZkSyncPriorityOp::Deposit(deposit) => (deposit.amount, deposit.token),
                ZkSyncPriorityOp::FullExit(other) => {
                    panic!("Incorrect input for DepositingBalances: {:?}", other);
                }
            };

            let token_symbol = tokens
                .token_symbol(storage, token_id)
                .await?
                .ok_or_else(|| unable_to_find_token(token_id))?;

            let expected_accept_block = confirmations_for_eth_event + (received_on_block as u32);

            let balance = balances
                .entry(token_symbol)
                .or_insert_with(DepositingFunds::default);

            balance.amount.0 += amount;

            // `balance.expected_accept_block` should be the greatest block number among
            // all the deposits for a certain token.
            if expected_accept_block > balance.expected_accept_block {
                balance.expected_accept_block = expected_accept_block;
            }
        }

        Ok(DepositingBalances { balances })
    }

    pub fn validate_receipts_query(
        query: AccountReceiptsQuery,
    ) -> Result<(TxLocation, SearchDirection, BlockNumber), ApiError> {
        if *query.limit == 0 && *query.limit > MAX_LIMIT {
            return Err(ApiError::bad_request("Incorrect limit")
                .detail(format!("Limit should be between {} and {}", 1, MAX_LIMIT)));
        }

        let (location, direction) = match (query.block, query.index, query.direction) {
            // Just try to fetch latest transactions.
            (None, None, None) => (
                TxLocation {
                    block: BlockNumber(u32::MAX),
                    index: None,
                },
                SearchDirection::Older,
            ),
            (Some(block), index, Some(direction)) => (TxLocation { block, index }, direction),

            _ => {
                return Err(ApiError::bad_request("Incorrect transaction location")
                    .detail("All parameters must be passed: block, index, direction."))
            }
        };

        Ok((location, direction, query.limit))
    }

    pub fn tx_receipt_from_response(inner: AccountTxReceiptResponse) -> AccountTxReceipt {
        let block = BlockNumber(inner.block_number as u32);
        let index = inner.block_index.map(|x| x as u32);
        let hash = TxHash::from_slice(&inner.tx_hash).unwrap_or_else(|| {
            panic!(
                "Database provided an incorrect tx_hash field: {}",
                hex::encode(&inner.tx_hash)
            )
        });

        if !inner.success {
            return AccountTxReceipt {
                index,
                hash,
                receipt: Receipt::Rejected {
                    reason: inner.fail_reason,
                },
            };
        }

        let receipt = match (
            inner.commit_tx_hash.is_some(),
            inner.verify_tx_hash.is_some(),
        ) {
            (false, false) => Receipt::Executed,
            (true, false) => Receipt::Committed { block },
            (true, true) => Receipt::Verified { block },
            (false, true) => panic!(
                "Database provided an incorrect account tx reciept: {:?}",
                inner
            ),
        };

        AccountTxReceipt {
            index,
            receipt,
            hash,
        }
    }

    pub fn op_receipt_from_response(inner: AccountOpReceiptResponse) -> AccountOpReceipt {
        let block = BlockNumber(inner.block_number as u32);
        let index = inner.block_index as u32;
        let hash = H256::from_slice(&inner.eth_hash);

        let receipt = match (
            inner.commit_tx_hash.is_some(),
            inner.verify_tx_hash.is_some(),
        ) {
            (false, false) => Receipt::Executed,
            (true, false) => Receipt::Committed { block },
            (true, true) => Receipt::Verified { block },
            (false, true) => panic!(
                "Database provided an incorrect account tx receipt: {:?}",
                inner
            ),
        };

        AccountOpReceipt {
            index,
            receipt,
            hash,
        }
    }

    pub fn pending_account_op_receipt_from_priority_op(op: PriorityOp) -> PendingAccountOpReceipt {
        PendingAccountOpReceipt {
            eth_block: op.eth_block,
            hash: op.eth_hash,
        }
    }
}
