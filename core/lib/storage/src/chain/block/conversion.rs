//! Module with additional conversion methods for the storage records.
//! These methods are only needed for the `block` module, so they're kept in a
//! private module.

// Built-in deps
use std::convert::TryFrom;
// External imports
// Workspace imports
use zksync_api_types::v02::transaction::{
    L1Transaction, Transaction, TransactionData, TxInBlockStatus,
};
use zksync_types::{
    aggregated_operations::AggregatedOperation,
    block::{ExecutedPriorityOp, ExecutedTx},
    tx::TxHash,
    BlockNumber, PriorityOp, SignedZkSyncTx, ZkSyncOp, ZkSyncTx, H256,
};
// Local imports
use crate::chain::operations::records::StoredAggregatedOperation;
use crate::utils::affected_accounts;
use crate::{
    chain::{
        block::records::TransactionItem,
        operations::records::{
            NewExecutedPriorityOperation, NewExecutedTransaction, StoredExecutedPriorityOperation,
            StoredExecutedTransaction,
        },
    },
    QueryResult, StorageProcessor,
};

impl StoredExecutedTransaction {
    pub(crate) fn into_executed_tx(self) -> ExecutedTx {
        let tx: ZkSyncTx = serde_json::from_value(self.tx).expect("Unparsable ZkSyncTx in db");
        let franklin_op: Option<ZkSyncOp> =
            serde_json::from_value(self.operation).expect("Unparsable ZkSyncOp in db");
        let eth_sign_data = self
            .eth_sign_data
            .map(|value| serde_json::from_value(value).expect("Unparsable EthSignData"));
        ExecutedTx {
            signed_tx: SignedZkSyncTx {
                tx,
                eth_sign_data,
                created_at: self.created_at,
            },
            success: self.success,
            op: franklin_op,
            fail_reason: self.fail_reason,
            block_index: self
                .block_index
                .map(|val| u32::try_from(val).expect("Invalid block index")),
            created_at: self.created_at,
            batch_id: self.batch_id,
        }
    }
}

impl StoredExecutedPriorityOperation {
    pub fn into_executed(self) -> ExecutedPriorityOp {
        let franklin_op: ZkSyncOp =
            serde_json::from_value(self.operation).expect("Unparsable priority op in db");
        ExecutedPriorityOp {
            priority_op: PriorityOp {
                serial_id: self.priority_op_serialid as u64,
                data: franklin_op
                    .try_get_priority_op()
                    .expect("ZkSyncOp should have priority op"),
                deadline_block: self.deadline_block as u64,
                eth_hash: H256::from_slice(&self.eth_hash),
                eth_block: self.eth_block as u64,
                eth_block_index: self.eth_block_index.map(|index| index as u64),
            },
            op: franklin_op,
            block_index: self.block_index as u32,
            created_at: self.created_at,
        }
    }
}

impl NewExecutedPriorityOperation {
    pub(crate) fn prepare_stored_priority_op(
        exec_prior_op: ExecutedPriorityOp,
        block: BlockNumber,
    ) -> Self {
        let operation = serde_json::to_value(&exec_prior_op.op).unwrap();
        let tx_hash = exec_prior_op.priority_op.tx_hash().as_ref().to_vec();

        let (from_account, to_account) = match exec_prior_op.op {
            ZkSyncOp::Deposit(deposit) => (deposit.priority_op.from, deposit.priority_op.to),
            ZkSyncOp::FullExit(full_exit) => {
                let eth_address = full_exit.priority_op.eth_address;
                (eth_address, eth_address)
            }
            _ => panic!(
                "Incorrect type of priority op: {:?}",
                exec_prior_op.priority_op
            ),
        };

        let affected_accounts = exec_prior_op
            .priority_op
            .data
            .affected_accounts()
            .into_iter()
            .map(|address| address.as_bytes().to_vec())
            .collect();
        let token = exec_prior_op.priority_op.data.token_id().0 as i32;

        Self {
            block_number: i64::from(*block),
            block_index: exec_prior_op.block_index as i32,
            operation,
            from_account: from_account.as_ref().to_vec(),
            to_account: to_account.as_ref().to_vec(),
            priority_op_serialid: exec_prior_op.priority_op.serial_id as i64,
            deadline_block: exec_prior_op.priority_op.deadline_block as i64,
            eth_hash: exec_prior_op.priority_op.eth_hash.as_bytes().to_vec(),
            eth_block: exec_prior_op.priority_op.eth_block as i64,
            created_at: exec_prior_op.created_at,
            eth_block_index: exec_prior_op
                .priority_op
                .eth_block_index
                .map(|index| index as i64),
            tx_hash,
            affected_accounts,
            token,
        }
    }
}

impl NewExecutedTransaction {
    pub(crate) async fn prepare_stored_tx(
        exec_tx: ExecutedTx,
        block: BlockNumber,
        storage: &mut StorageProcessor<'_>,
    ) -> QueryResult<Self> {
        fn cut_prefix(input: &str) -> String {
            if let Some(input) = input.strip_prefix("0x") {
                input.into()
            } else if let Some(input) = input.strip_prefix("sync:") {
                input.into()
            } else {
                input.into()
            }
        }

        let tx = serde_json::to_value(&exec_tx.signed_tx.tx).expect("Cannot serialize tx");
        let operation = serde_json::to_value(&exec_tx.op).expect("Cannot serialize operation");

        let (from_account_hex, to_account_hex): (String, Option<String>) =
            match exec_tx.signed_tx.tx {
                ZkSyncTx::Withdraw(_) | ZkSyncTx::Transfer(_) | ZkSyncTx::WithdrawNFT(_) => (
                    serde_json::from_value(tx["from"].clone()).unwrap(),
                    serde_json::from_value(tx["to"].clone()).unwrap(),
                ),
                ZkSyncTx::ChangePubKey(_) => (
                    serde_json::from_value(tx["account"].clone()).unwrap(),
                    serde_json::from_value(tx["newPkHash"].clone()).unwrap(),
                ),
                ZkSyncTx::Close(_) => (
                    serde_json::from_value(tx["account"].clone()).unwrap(),
                    serde_json::from_value(tx["account"].clone()).unwrap(),
                ),
                ZkSyncTx::ForcedExit(_) => (
                    serde_json::from_value(tx["target"].clone()).unwrap(),
                    serde_json::from_value(tx["target"].clone()).unwrap(),
                ),
                ZkSyncTx::MintNFT(_) => (
                    serde_json::from_value(tx["creatorAddress"].clone()).unwrap(),
                    serde_json::from_value(tx["recipient"].clone()).unwrap(),
                ),
                ZkSyncTx::Swap(_) => (
                    serde_json::from_value(tx["submitterAddress"].clone()).unwrap(),
                    serde_json::from_value(tx["submitterAddress"].clone()).unwrap(),
                ),
            };

        let from_account: Vec<u8> = hex::decode(cut_prefix(&from_account_hex)).unwrap();
        let to_account: Option<Vec<u8>> =
            to_account_hex.map(|value| hex::decode(cut_prefix(&value)).unwrap());

        let eth_sign_data = exec_tx.signed_tx.eth_sign_data.as_ref().map(|sign_data| {
            serde_json::to_value(sign_data).expect("Failed to encode EthSignData")
        });

        let affected_accounts = affected_accounts(&exec_tx.signed_tx.tx, storage)
            .await?
            .into_iter()
            .map(|address| address.as_bytes().to_vec())
            .collect();
        let used_tokens = exec_tx
            .signed_tx
            .tx
            .tokens()
            .into_iter()
            .map(|id| id.0 as i32)
            .collect();
        Ok(Self {
            block_number: i64::from(*block),
            tx_hash: exec_tx.signed_tx.hash().as_ref().to_vec(),
            from_account,
            to_account,
            tx,
            operation,
            success: exec_tx.success,
            fail_reason: exec_tx.fail_reason,
            block_index: exec_tx.block_index.map(|idx| idx as i32),
            primary_account_address: exec_tx.signed_tx.account().as_bytes().to_vec(),
            nonce: *exec_tx.signed_tx.nonce() as i64,
            created_at: exec_tx.created_at,
            eth_sign_data,
            batch_id: exec_tx.batch_id,
            affected_accounts,
            used_tokens,
        })
    }
}

impl StoredAggregatedOperation {
    pub(crate) fn into_aggregated_op(self) -> (i64, AggregatedOperation) {
        (
            self.id,
            serde_json::from_value(self.arguments)
                .expect("Incorrect serialized aggregated operation in storage"),
        )
    }
}

impl TransactionItem {
    pub(crate) fn transaction_from_item(
        item: TransactionItem,
        is_block_finalized: bool,
    ) -> Transaction {
        let tx_hash = TxHash::from_slice(&item.tx_hash).unwrap();
        let block_number = Some(BlockNumber(item.block_number as u32));
        let status = if item.success {
            if is_block_finalized {
                TxInBlockStatus::Finalized
            } else {
                TxInBlockStatus::Committed
            }
        } else {
            TxInBlockStatus::Rejected
        };
        let op = if let Some(eth_hash) = item.eth_hash {
            let eth_hash = H256::from_slice(&eth_hash);
            let id = item.priority_op_serialid.unwrap() as u64;
            let operation: ZkSyncOp = serde_json::from_value(item.op).unwrap();
            TransactionData::L1(
                L1Transaction::from_executed_op(operation, eth_hash, id, tx_hash).unwrap(),
            )
        } else {
            TransactionData::L2(serde_json::from_value(item.op).unwrap())
        };

        Transaction {
            tx_hash,
            block_index: item.block_index.map(|i| i as u32),
            block_number,
            op,
            status,
            fail_reason: item.fail_reason,
            created_at: Some(item.created_at),
            batch_id: item.batch_id.map(|id| id as u32),
        }
    }
}

/// Helper method for `find_block_by_height_or_hash`. It checks whether
/// provided string can be interpreted like a hash, and if so, returns the
/// hexadecimal string without prefix.
pub(crate) fn decode_hex_with_prefix(query: &str) -> Option<Vec<u8>> {
    const HASH_STRING_SIZE: usize = 32 * 2; // 32 bytes, 2 symbols per byte.

    let query = if let Some(query) = query.strip_prefix("0x") {
        query
    } else if let Some(query) = query.strip_prefix("sync-bl:") {
        query
    } else {
        query
    };

    if query.len() != HASH_STRING_SIZE {
        return None;
    }

    hex::decode(query).ok()
}

#[cfg(test)]
mod test {
    use crate::chain::block::conversion::decode_hex_with_prefix;
    use zksync_types::{H160, H256};

    fn check_all_prefixes(value: String) -> bool {
        let string_wit_0x_prefix = format!("0x{}", &value);
        let string_wit_sync_bl_prefix = format!("sync-bl:{}", &value);
        decode_hex_with_prefix(&string_wit_0x_prefix).is_some()
            && decode_hex_with_prefix(&string_wit_sync_bl_prefix).is_some()
            && decode_hex_with_prefix(&value).is_some()
    }

    #[test]
    fn test_decode_hex() {
        let correct_string = hex::encode(H256::random());
        assert!(check_all_prefixes(correct_string));
        let short_string = hex::encode(H160::random());
        assert!(!check_all_prefixes(short_string));
        let mut incorrect_string = hex::encode(H256::random());
        // 'x' is impossible to use in hex string
        incorrect_string.replace_range(10..11, "x");
        assert!(!check_all_prefixes(incorrect_string));
        let incorrect_string2 = "random_string".to_string();
        assert!(!check_all_prefixes(incorrect_string2));
    }
}
