// Built-in uses
use ethabi::Address;
use std::time::Instant;
// External uses
use jsonrpc_core::{Error, Result};
// Workspace uses
use zksync_crypto::convert::FeConvert;
use zksync_storage::{
    chain::{block::records::StorageBlock, operations_ext::records::Web3TxReceipt},
    StorageProcessor,
};
use zksync_types::withdrawals::WithdrawalPendingEvent;
use zksync_types::{ExecutedOperations, TokenId, ZkSyncOp};
// Local uses
use super::{
    converter::{resolve_block_number, transaction_from_tx_data, u256_from_biguint},
    types::{
        BlockInfo, BlockNumber, Bytes, CallRequest, CommonLogData, Filter, Log, Transaction,
        TransactionReceipt, TxData, H160, H2048, H256, U256, U64,
    },
    Web3RpcApp,
};

impl Web3RpcApp {
    pub async fn _impl_block_number(self) -> Result<U64> {
        let start = Instant::now();
        let mut storage = self.access_storage().await?;
        let block_number = storage
            .chain()
            .block_schema()
            .get_last_verified_confirmed_block()
            .await
            .map_err(|_| Error::internal_error())?;

        metrics::histogram!("api", start.elapsed(), "type" => "web3", "endpoint_name" => "block_number");
        Ok(U64::from(block_number.0))
    }

    pub async fn _impl_get_balance(
        self,
        address: H160,
        block: Option<BlockNumber>,
    ) -> Result<U256> {
        let start = Instant::now();
        let mut storage = self.access_storage().await?;
        let mut transaction = storage
            .start_transaction()
            .await
            .map_err(|_| Error::internal_error())?;
        let block_number = resolve_block_number(&mut transaction, block)
            .await?
            .ok_or_else(|| Error::invalid_params("Block with such number doesn't exist yet"))?;
        let balance = transaction
            .chain()
            .account_schema()
            .get_account_balance_for_block(address, block_number, TokenId(0))
            .await
            .map_err(|_| Error::internal_error())?;
        let result = u256_from_biguint(balance);
        metrics::histogram!("api", start.elapsed(), "type" => "web3", "endpoint_name" => "get_balance");
        Ok(result)
    }

    pub async fn _impl_get_block_transaction_count_by_hash(
        self,
        hash: H256,
    ) -> Result<Option<U256>> {
        let start = Instant::now();
        let mut storage = self.access_storage().await?;
        let mut transaction = storage
            .start_transaction()
            .await
            .map_err(|_| Error::internal_error())?;

        let block_number = transaction
            .chain()
            .block_schema()
            .get_block_number_by_hash(hash.as_bytes())
            .await
            .map_err(|_| Error::internal_error())?;
        let result = match block_number {
            Some(block_number) => {
                Some(Self::block_transaction_count(&mut transaction, block_number).await?)
            }
            None => None,
        };
        transaction
            .commit()
            .await
            .map_err(|_| Error::internal_error())?;

        metrics::histogram!("api", start.elapsed(), "type" => "web3", "endpoint_name" => "get_block_transaction_count_by_hash");
        Ok(result)
    }

    pub async fn _impl_get_block_transaction_count_by_number(
        self,
        block: Option<BlockNumber>,
    ) -> Result<Option<U256>> {
        let start = Instant::now();
        let mut storage = self.access_storage().await?;
        let mut transaction = storage
            .start_transaction()
            .await
            .map_err(|_| Error::internal_error())?;

        let block_number = resolve_block_number(&mut transaction, block).await?;
        let result = match block_number {
            Some(block_number) => {
                Some(Self::block_transaction_count(&mut transaction, block_number).await?)
            }
            None => None,
        };
        transaction
            .commit()
            .await
            .map_err(|_| Error::internal_error())?;

        metrics::histogram!("api", start.elapsed(), "type" => "web3", "endpoint_name" => "get_block_transaction_count_by_number");
        Ok(result)
    }

    pub async fn _impl_get_transaction_by_hash(self, hash: H256) -> Result<Option<Transaction>> {
        let start = Instant::now();
        let mut storage = self.access_storage().await?;

        let tx = storage
            .chain()
            .operations_ext_schema()
            .tx_data_for_web3(hash.as_ref())
            .await
            .map_err(|_| Error::internal_error())?;
        let result = tx.map(|tx| transaction_from_tx_data(tx.into()));

        metrics::histogram!("api", start.elapsed(), "type" => "web3", "endpoint_name" => "get_transaction_by_hash");
        Ok(result)
    }

    pub async fn _impl_get_block_by_number(
        self,
        block_number: Option<BlockNumber>,
        include_txs: bool,
    ) -> Result<Option<BlockInfo>> {
        let start = Instant::now();
        let mut storage = self.access_storage().await?;
        let mut transaction = storage
            .start_transaction()
            .await
            .map_err(|_| Error::internal_error())?;

        let block_number = resolve_block_number(&mut transaction, block_number).await?;
        let result = match block_number {
            Some(block_number) => {
                Some(Self::block_by_number(&mut transaction, block_number, include_txs).await?)
            }
            None => None,
        };
        transaction
            .commit()
            .await
            .map_err(|_| Error::internal_error())?;

        metrics::histogram!("api", start.elapsed(), "type" => "web3", "endpoint_name" => "get_block_by_number");
        Ok(result)
    }

    pub async fn _impl_get_block_by_hash(
        self,
        hash: H256,
        include_txs: bool,
    ) -> Result<Option<BlockInfo>> {
        let start = Instant::now();
        let mut storage = self.access_storage().await?;
        let mut transaction = storage
            .start_transaction()
            .await
            .map_err(|_| Error::internal_error())?;

        let block_number = transaction
            .chain()
            .block_schema()
            .get_block_number_by_hash(hash.as_bytes())
            .await
            .map_err(|_| Error::internal_error())?;
        let result = match block_number {
            Some(block_number) => {
                Some(Self::block_by_number(&mut transaction, block_number, include_txs).await?)
            }
            None => None,
        };
        transaction
            .commit()
            .await
            .map_err(|_| Error::internal_error())?;

        metrics::histogram!("api", start.elapsed(), "type" => "web3", "endpoint_name" => "get_block_by_hash");
        Ok(result)
    }

    pub async fn _impl_get_transaction_receipt(
        self,
        hash: H256,
    ) -> Result<Option<TransactionReceipt>> {
        let start = Instant::now();
        let mut storage = self.access_storage().await?;

        let tx = storage
            .chain()
            .operations_ext_schema()
            .web3_receipt_by_hash(hash.as_ref())
            .await
            .map_err(|_| Error::internal_error())?;
        let result = if let Some(tx) = tx {
            Some(self.tx_receipt(&mut storage, tx).await?)
        } else {
            None
        };

        metrics::histogram!("api", start.elapsed(), "type" => "web3", "endpoint_name" => "get_transaction_receipt");
        Ok(result)
    }

    pub async fn _impl_get_logs(self, filter: Filter) -> Result<Vec<Log>> {
        let start = Instant::now();

        let mut storage = self.access_storage().await?;
        let mut transaction = storage
            .start_transaction()
            .await
            .map_err(|_| Error::internal_error())?;

        let from_block = resolve_block_number(&mut transaction, filter.from_block).await?;
        let to_block = resolve_block_number(&mut transaction, filter.to_block).await?;

        let (from_block, to_block) = match (from_block, to_block) {
            (Some(from_block), Some(to_block)) => (from_block, to_block),
            _ => {
                return Err(Error::invalid_params(
                    "Block with such number doesn't exist yet",
                ));
            }
        };

        if from_block > to_block {
            return Err(Error::invalid_params(
                "`fromBlock` must not be greater than `toBlock`",
            ));
        }
        if to_block.0 - from_block.0 > self.max_block_range {
            return Err(Error::invalid_params(format!(
                "The difference between `toBlock` and `fromBlock` must not be greater than {}",
                self.max_block_range
            )));
        }

        let topics = if let Some(mut topics) = filter.topics {
            // If there is non-null topic at the non-first position then return empty vec,
            // since all our logs contain exactly one topic.
            let has_not_first = topics
                .iter()
                .enumerate()
                .any(|(i, topic)| i > 0 && topic.is_some());
            if has_not_first {
                return Ok(Vec::new());
            } else if topics.is_empty() {
                Vec::new()
            } else {
                topics.remove(0).unwrap_or_default().0
            }
        } else {
            Vec::new()
        };
        let addresses = filter.address.map(|a| a.0).unwrap_or_default();
        let mut result = Vec::new();

        let receipts = transaction
            .chain()
            .operations_ext_schema()
            .web3_receipts(from_block, to_block)
            .await
            .map_err(|_| Error::internal_error())?;
        for receipt in receipts {
            let logs = self.logs_from_receipt(&mut transaction, receipt).await?;
            let filtered = logs.into_iter().filter(|log| {
                if !topics.is_empty() && !topics.contains(&log.topics[0]) {
                    return false;
                }
                if !addresses.is_empty() && !addresses.contains(&log.address) {
                    return false;
                }
                true
            });
            result.extend(filtered);
        }

        transaction
            .commit()
            .await
            .map_err(|_| Error::internal_error())?;

        metrics::histogram!("api", start.elapsed(), "type" => "web3", "endpoint_name" => "get_logs");
        Ok(result)
    }

    pub async fn _impl_call(self, req: CallRequest, _block: Option<BlockNumber>) -> Result<Bytes> {
        let start = Instant::now();
        let mut storage = self.access_storage().await?;

        let result = self
            .calls_helper
            .execute(&mut storage, req.to, req.data.unwrap_or_default().0)
            .await;

        metrics::histogram!("api", start.elapsed(), "type" => "web3", "endpoint_name" => "call");
        result.map(Bytes)
    }

    pub async fn _impl_check_withdrawal(
        self,
        tx_hash: H256,
    ) -> Result<Vec<WithdrawalPendingEvent>> {
        let start = Instant::now();
        let mut storage = self.access_storage().await?;

        let withdrawals = storage
            .withdrawals_schema()
            .get_finalized_withdrawals(tx_hash)
            .await
            .map_err(|_| Error::internal_error())?;

        metrics::histogram!("api", start.elapsed(), "type" => "web3", "endpoint_name" => "check_withdrawal");
        Ok(withdrawals)
    }

    pub(crate) async fn logs_from_receipt(
        &self,
        storage: &mut StorageProcessor<'_>,
        receipt: Web3TxReceipt,
    ) -> Result<Vec<Log>> {
        let common_data = CommonLogData {
            block_hash: Some(H256::from_slice(&receipt.block_hash)),
            block_number: Some(receipt.block_number.into()),
            transaction_hash: H256::from_slice(&receipt.tx_hash),
            // U64::MAX for failed transactions
            transaction_index: Some(receipt.block_index.map(Into::into).unwrap_or(U64::MAX)),
        };
        let op: Option<ZkSyncOp> = serde_json::from_value(receipt.operation).unwrap();
        let mut logs = Vec::new();
        if let Some(op) = op {
            let zksync_log = self
                .logs_helper
                .zksync_log(op.clone(), common_data, storage)
                .await?;
            if let Some(zksync_log) = zksync_log {
                logs.push(zksync_log);
            }

            let erc_logs = self.logs_helper.erc_logs(op, common_data, storage).await?;
            logs.extend(erc_logs);
        }
        Ok(logs)
    }

    pub(crate) async fn tx_receipt(
        &self,
        storage: &mut StorageProcessor<'_>,
        receipt: Web3TxReceipt,
    ) -> Result<TransactionReceipt> {
        let logs = self.logs_from_receipt(storage, receipt.clone()).await?;
        let root_hash = H256::from_slice(&receipt.block_hash);
        Ok(TransactionReceipt {
            transaction_hash: H256::from_slice(&receipt.tx_hash),
            // U64::MAX for failed transactions
            transaction_index: receipt.block_index.map(Into::into).unwrap_or(U64::MAX),
            block_hash: Some(root_hash),
            block_number: Some(receipt.block_number.into()),
            from: Address::from_slice(&receipt.from_account),
            to: receipt.to_account.map(|acc| Address::from_slice(&acc)),
            cumulative_gas_used: 0.into(),
            gas_used: Some(0.into()),
            contract_address: None,
            logs,
            status: Some((receipt.success as u8).into()),
            root: Some(root_hash),
            logs_bloom: H2048::zero(),
            transaction_type: None,
            effective_gas_price: None,
        })
    }

    async fn storage_block(
        storage: &mut StorageProcessor<'_>,
        block_number: zksync_types::BlockNumber,
    ) -> Result<Option<StorageBlock>> {
        let block = storage
            .chain()
            .block_schema()
            .get_storage_block(block_number)
            .await
            .map_err(|_| Error::internal_error())?;
        Ok(block)
    }

    pub(crate) async fn block_by_number(
        storage: &mut StorageProcessor<'_>,
        block_number: zksync_types::BlockNumber,
        include_txs: bool,
    ) -> Result<BlockInfo> {
        let mut transaction = storage
            .start_transaction()
            .await
            .map_err(|_| Error::internal_error())?;

        let parent_hash = if block_number.0 == 0 {
            H256::zero()
        } else {
            // It was already checked that the block is in storage, so the parent block has to be there too.
            let block = Self::storage_block(&mut transaction, block_number - 1)
                .await?
                .ok_or_else(Error::internal_error)?;
            H256::from_slice(&block.root_hash)
        };

        let result = if include_txs {
            // It was already checked that the block is in storage.
            let block = transaction
                .chain()
                .block_schema()
                .get_block(block_number)
                .await
                .map_err(|_| Error::internal_error())?
                .ok_or_else(Error::internal_error)?;
            let hash = H256::from_slice(&block.new_root_hash.to_bytes());
            let transactions = block
                .block_transactions
                .into_iter()
                .map(|tx| {
                    let tx = match tx {
                        ExecutedOperations::Tx(tx) => TxData {
                            block_hash: hash,
                            block_number: block_number.0,
                            block_index: tx.block_index,
                            from: tx.signed_tx.tx.from_account(),
                            to: tx.signed_tx.tx.to_account(),
                            nonce: tx.signed_tx.tx.nonce().0,
                            tx_hash: H256::from_slice(tx.signed_tx.tx.hash().as_ref()),
                        },
                        ExecutedOperations::PriorityOp(op) => TxData {
                            block_hash: hash,
                            block_number: block_number.0,
                            block_index: Some(op.block_index),
                            from: op.priority_op.data.from_account(),
                            to: Some(op.priority_op.data.to_account()),
                            nonce: op.priority_op.serial_id as u32,
                            tx_hash: H256::from_slice(op.priority_op.tx_hash().as_ref()),
                        },
                    };
                    transaction_from_tx_data(tx)
                })
                .collect();

            BlockInfo::new_with_txs(
                hash,
                parent_hash,
                block_number,
                block.timestamp,
                transactions,
            )
        } else {
            // It was already checked that the block is in storage.
            let block = Self::storage_block(&mut transaction, block_number)
                .await?
                .ok_or_else(Error::internal_error)?;
            let hashes = transaction
                .chain()
                .block_schema()
                .get_block_transactions_hashes(block_number)
                .await
                .map_err(|_| Error::internal_error())?
                .into_iter()
                .map(|hash| H256::from_slice(&hash))
                .collect();

            BlockInfo::new_with_hashes(
                H256::from_slice(&block.root_hash),
                parent_hash,
                block_number,
                block.timestamp.unwrap_or_default() as u64,
                hashes,
            )
        };
        transaction
            .commit()
            .await
            .map_err(|_| Error::internal_error())?;
        Ok(result)
    }

    async fn block_transaction_count(
        storage: &mut StorageProcessor<'_>,
        block_number: zksync_types::BlockNumber,
    ) -> Result<U256> {
        let count = storage
            .chain()
            .block_schema()
            .get_block_transactions_count(block_number)
            .await
            .map_err(|_| Error::internal_error())?;
        Ok(U256::from(count))
    }
}
