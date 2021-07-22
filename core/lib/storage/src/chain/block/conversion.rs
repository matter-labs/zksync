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
    Action, ActionType, BlockNumber, Operation, PriorityOp, SignedZkSyncTx, ZkSyncOp, ZkSyncTx,
    H256,
};
// Local imports
use crate::chain::operations::records::StoredAggregatedOperation;
use crate::{
    chain::{
        block::{records::TransactionItem, BlockSchema},
        operations::records::{
            NewExecutedPriorityOperation, NewExecutedTransaction, StoredExecutedPriorityOperation,
            StoredExecutedTransaction, StoredOperation,
        },
    },
    prover::ProverSchema,
    QueryResult, StorageActionType, StorageProcessor,
};

impl StoredOperation {
    pub async fn into_op(self, conn: &mut StorageProcessor<'_>) -> QueryResult<Operation> {
        let block_number = BlockNumber(self.block_number as u32);
        let id = Some(self.id);

        let action = if self.action_type == StorageActionType::from(ActionType::COMMIT) {
            Action::Commit
        } else if self.action_type == StorageActionType::from(ActionType::VERIFY) {
            let proof = Box::new(ProverSchema(conn).load_proof(block_number).await?);
            Action::Verify {
                proof: proof.expect("No proof for verify action").into(),
            }
        } else {
            unreachable!("Incorrect action type in db");
        };

        let block = BlockSchema(conn)
            .get_block(block_number)
            .await?
            .expect("Block for action does not exist");

        Ok(Operation { id, action, block })
    }
}

impl StoredExecutedTransaction {
    pub fn into_executed_tx(self) -> Result<ExecutedTx, anyhow::Error> {
        let tx: ZkSyncTx = serde_json::from_value(self.tx).expect("Unparsable ZkSyncTx in db");
        let franklin_op: Option<ZkSyncOp> =
            serde_json::from_value(self.operation).expect("Unparsable ZkSyncOp in db");
        let eth_sign_data = self
            .eth_sign_data
            .map(|value| serde_json::from_value(value).expect("Unparsable EthSignData"));
        Ok(ExecutedTx {
            signed_tx: SignedZkSyncTx { tx, eth_sign_data },
            success: self.success,
            op: franklin_op,
            fail_reason: self.fail_reason,
            block_index: self
                .block_index
                .map(|val| u32::try_from(val).expect("Invalid block index")),
            created_at: self.created_at,
            batch_id: self.batch_id,
        })
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
    pub fn prepare_stored_priority_op(
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
        }
    }
}

impl NewExecutedTransaction {
    pub fn prepare_stored_tx(exec_tx: ExecutedTx, block: BlockNumber) -> Self {
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
                    serde_json::from_value(tx["recipientAddress"].clone()).unwrap(),
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

        Self {
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
        }
    }
}

impl StoredAggregatedOperation {
    pub fn into_aggregated_op(self) -> (i64, AggregatedOperation) {
        (
            self.id,
            serde_json::from_value(self.arguments)
                .expect("Incorrect serialized aggregated operation in storage"),
        )
    }
}

impl TransactionItem {
    pub fn transaction_from_item(item: TransactionItem, is_block_finalized: bool) -> Transaction {
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
            block_number,
            op,
            status,
            fail_reason: item.fail_reason,
            created_at: Some(item.created_at),
        }
    }
}
