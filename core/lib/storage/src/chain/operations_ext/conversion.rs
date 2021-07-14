// Built-in deps
// External imports
// Workspace imports
use zksync_api_types::v02::transaction::{
    ForcedExitData, L1Receipt, L1Transaction, L2Receipt, L2Transaction, Receipt, Transaction,
    TransactionData, TxData, TxInBlockStatus, WithdrawData, WithdrawNFTData,
};
use zksync_types::{
    tx::{EthSignData, TxHash},
    BlockNumber, EthBlockId, ZkSyncOp, ZkSyncTx, H256,
};
// Local imports
use super::records::{StorageTxData, StorageTxReceipt};

impl StorageTxReceipt {
    pub fn receipt_from_storage_receipt(
        receipt: StorageTxReceipt,
        is_block_finalized: Option<bool>,
    ) -> Receipt {
        if receipt.block_number.is_some() {
            let status = if receipt.success.unwrap() {
                if is_block_finalized.unwrap() {
                    TxInBlockStatus::Finalized
                } else {
                    TxInBlockStatus::Committed
                }
            } else {
                TxInBlockStatus::Rejected
            };
            if receipt.eth_block.is_some() {
                Receipt::L1(L1Receipt {
                    status,
                    eth_block: EthBlockId(receipt.eth_block.unwrap() as u64),
                    rollup_block: receipt
                        .block_number
                        .map(|number| BlockNumber(number as u32)),
                    id: receipt.priority_op_serialid.unwrap() as u64,
                })
            } else {
                Receipt::L2(L2Receipt {
                    status,
                    tx_hash: TxHash::from_slice(&receipt.tx_hash).unwrap(),
                    rollup_block: receipt
                        .block_number
                        .map(|number| BlockNumber(number as u32)),
                    fail_reason: receipt.fail_reason,
                })
            }
        } else {
            Receipt::L2(L2Receipt {
                status: TxInBlockStatus::Queued,
                tx_hash: TxHash::from_slice(&receipt.tx_hash).unwrap(),
                rollup_block: None,
                fail_reason: None,
            })
        }
    }
}

impl StorageTxData {
    pub fn tx_data_from_zksync_tx(
        tx: ZkSyncTx,
        complete_withdrawals_tx_hash: Option<H256>,
    ) -> TransactionData {
        let tx = match tx {
            ZkSyncTx::ChangePubKey(tx) => L2Transaction::ChangePubKey(tx),
            ZkSyncTx::Close(tx) => L2Transaction::Close(tx),
            ZkSyncTx::ForcedExit(tx) => L2Transaction::ForcedExit(Box::new(ForcedExitData {
                tx: *tx,
                eth_tx_hash: complete_withdrawals_tx_hash,
            })),
            ZkSyncTx::Transfer(tx) => L2Transaction::Transfer(tx),
            ZkSyncTx::Withdraw(tx) => L2Transaction::Withdraw(Box::new(WithdrawData {
                tx: *tx,
                eth_tx_hash: complete_withdrawals_tx_hash,
            })),
            ZkSyncTx::MintNFT(tx) => L2Transaction::MintNFT(tx),
            ZkSyncTx::WithdrawNFT(tx) => L2Transaction::WithdrawNFT(Box::new(WithdrawNFTData {
                tx: *tx,
                eth_tx_hash: complete_withdrawals_tx_hash,
            })),
            ZkSyncTx::Swap(tx) => L2Transaction::Swap(tx),
        };
        TransactionData::L2(tx)
    }

    pub fn data_from_storage_data(
        data: StorageTxData,
        is_block_finalized: Option<bool>,
        complete_withdrawals_tx_hash: Option<H256>,
    ) -> TxData {
        let tx_hash = TxHash::from_slice(&data.tx_hash).unwrap();
        let tx = if data.block_number.is_some() {
            let block_number = data.block_number.map(|number| BlockNumber(number as u32));
            let status = if data.success.unwrap() {
                if is_block_finalized.unwrap() {
                    TxInBlockStatus::Finalized
                } else {
                    TxInBlockStatus::Committed
                }
            } else {
                TxInBlockStatus::Rejected
            };

            let op = if data.eth_hash.is_some() {
                let operation: ZkSyncOp = serde_json::from_value(data.op).unwrap();
                let eth_hash = H256::from_slice(&data.eth_hash.unwrap());
                let id = data.priority_op_serialid.unwrap() as u64;
                TransactionData::L1(
                    L1Transaction::from_executed_op(operation, eth_hash, id, tx_hash).unwrap(),
                )
            } else {
                Self::tx_data_from_zksync_tx(
                    serde_json::from_value(data.op).unwrap(),
                    complete_withdrawals_tx_hash,
                )
            };
            Transaction {
                tx_hash,
                block_number,
                op,
                status,
                fail_reason: data.fail_reason,
                created_at: Some(data.created_at),
            }
        } else {
            let tx_data = Self::tx_data_from_zksync_tx(
                serde_json::from_value(data.op).unwrap(),
                complete_withdrawals_tx_hash,
            );
            Transaction {
                tx_hash,
                block_number: None,
                op: tx_data,
                status: TxInBlockStatus::Queued,
                fail_reason: None,
                created_at: Some(data.created_at),
            }
        };
        let eth_signature = data.eth_sign_data.map(|eth_sign_data| {
            let eth_sign_data: EthSignData = serde_json::from_value(eth_sign_data).unwrap();
            eth_sign_data.signature.to_string()
        });
        TxData { tx, eth_signature }
    }
}
