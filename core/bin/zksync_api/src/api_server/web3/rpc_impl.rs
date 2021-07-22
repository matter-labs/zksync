// Built-in uses
use std::collections::HashSet;
use std::iter::FromIterator;
use std::time::Instant;
// External uses
use jsonrpc_core::{Error, Result};
// Workspace uses
use zksync_crypto::convert::FeConvert;
use zksync_storage::{chain::operations_ext::records::Web3TxReceipt, StorageProcessor};
use zksync_types::{ExecutedOperations, ZkSyncOp};
// Local uses
use super::{
    converter::{resolve_block_number, transaction_from_tx_data, u256_from_biguint},
    types::{
        BlockInfo, BlockNumber, CommonLogData, Event, Log, Transaction, TransactionReceipt, TxData,
        ValueOrArray, H160, H2048, H256, U256, U64,
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
        metrics::histogram!("api.web3.block_number", start.elapsed());
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
            .get_account_eth_balance_for_block(address, block_number)
            .await
            .map_err(|_| Error::internal_error())?;
        let result = u256_from_biguint(balance)?;
        metrics::histogram!("api.web3.get_balance", start.elapsed());
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

        metrics::histogram!(
            "api.web3.get_block_transaction_count_by_hash",
            start.elapsed()
        );
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

        metrics::histogram!(
            "api.web3.get_block_transaction_count_by_number",
            start.elapsed()
        );
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

        metrics::histogram!("api.web3.get_transaction_by_hash", start.elapsed());
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

        metrics::histogram!("api.web3.get_block_by_number", start.elapsed());
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

        metrics::histogram!("api.web3.get_block_by_hash", start.elapsed());
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

        metrics::histogram!("api.web3.get_transaction_receipt", start.elapsed());
        Ok(result)
    }

    pub async fn _impl_get_logs(
        self,
        from_block: Option<BlockNumber>,
        to_block: Option<BlockNumber>,
        address: Option<ValueOrArray<H160>>,
        topics: Option<Vec<Option<ValueOrArray<H256>>>>,
    ) -> Result<Vec<Log>> {
        let start = Instant::now();

        let mut storage = self.access_storage().await?;
        let mut transaction = storage
            .start_transaction()
            .await
            .map_err(|_| Error::internal_error())?;

        let from_block = resolve_block_number(&mut transaction, from_block).await?;
        let to_block = resolve_block_number(&mut transaction, to_block).await?;

        let (from_block, to_block) = match (from_block, to_block) {
            (Some(from_block), Some(to_block)) => (from_block, to_block),
            _ => {
                return Err(Error::invalid_params(
                    "Block with such number doesn't exist yet",
                ));
            }
        };

        let topics = if let Some(mut topics) = topics {
            // If there is non-null topic at the non-first position then return empty vec,
            // since all our logs contain exactly one topic.
            let has_not_first = topics
                .iter()
                .enumerate()
                .any(|(i, topic)| i > 0 && topic.is_some());
            if has_not_first {
                return Ok(Vec::new());
            } else {
                topics.remove(0).unwrap_or_default().0
            }
        } else {
            Vec::new()
        };
        let addresses = address.map(|a| a.0).unwrap_or_default();
        let mut logs = Vec::new();
        match (addresses.is_empty(), topics.is_empty()) {
            (false, false) => {
                let (has_erc_topic, tx_types) = self.process_topics(topics);
                let (has_zksync_proxy_address, token_addresses) = self.process_addresses(addresses);

                if has_erc_topic {
                    let receipts = transaction
                        .chain()
                        .operations_ext_schema()
                        .web3_receipts_with_token_addresses(from_block, to_block, &token_addresses)
                        .await
                        .map_err(|_| Error::internal_error())?;
                    let token_addresses = HashSet::from_iter(token_addresses.into_iter());
                    for receipt in receipts {
                        self.append_logs(
                            &mut transaction,
                            receipt,
                            &mut logs,
                            false,
                            true,
                            Some(&token_addresses),
                        )
                        .await?;
                    }
                }

                if has_zksync_proxy_address {
                    let receipts = transaction
                        .chain()
                        .operations_ext_schema()
                        .web3_receipts_with_types(from_block, to_block, tx_types)
                        .await
                        .map_err(|_| Error::internal_error())?;
                    for receipt in receipts {
                        self.append_logs(&mut transaction, receipt, &mut logs, true, false, None)
                            .await?;
                    }
                }
            }
            (false, true) => {
                let (has_zksync_proxy_address, token_addresses) = self.process_addresses(addresses);
                if has_zksync_proxy_address {
                    let receipts = transaction
                        .chain()
                        .operations_ext_schema()
                        .web3_receipts(from_block, to_block)
                        .await
                        .map_err(|_| Error::internal_error())?;
                    for receipt in receipts {
                        self.append_logs(&mut transaction, receipt, &mut logs, true, false, None)
                            .await?;
                    }
                }
                let receipts = transaction
                    .chain()
                    .operations_ext_schema()
                    .web3_receipts_with_token_addresses(from_block, to_block, &token_addresses)
                    .await
                    .map_err(|_| Error::internal_error())?;
                let token_addresses = HashSet::from_iter(token_addresses.into_iter());
                for receipt in receipts {
                    self.append_logs(
                        &mut transaction,
                        receipt,
                        &mut logs,
                        false,
                        true,
                        Some(&token_addresses),
                    )
                    .await?;
                }
            }
            (true, false) => {
                let (has_erc_topic, tx_types) = self.process_topics(topics);
                if has_erc_topic {
                    let receipts = transaction
                        .chain()
                        .operations_ext_schema()
                        .web3_receipts(from_block, to_block)
                        .await
                        .map_err(|_| Error::internal_error())?;
                    for receipt in receipts {
                        self.append_logs(&mut transaction, receipt, &mut logs, false, true, None)
                            .await?;
                    }
                }
                let receipts = transaction
                    .chain()
                    .operations_ext_schema()
                    .web3_receipts_with_types(from_block, to_block, tx_types)
                    .await
                    .map_err(|_| Error::internal_error())?;
                for receipt in receipts {
                    self.append_logs(&mut transaction, receipt, &mut logs, true, false, None)
                        .await?;
                }
            }
            (true, true) => {
                let receipts = transaction
                    .chain()
                    .operations_ext_schema()
                    .web3_receipts(from_block, to_block)
                    .await
                    .map_err(|_| Error::internal_error())?;
                for receipt in receipts {
                    self.append_logs(&mut transaction, receipt, &mut logs, true, true, None)
                        .await?;
                }
            }
        };

        transaction
            .commit()
            .await
            .map_err(|_| Error::internal_error())?;

        metrics::histogram!("api.web3.get_logs", start.elapsed());
        Ok(logs)
    }

    async fn append_logs(
        &self,
        storage: &mut StorageProcessor<'_>,
        receipt: Web3TxReceipt,
        logs: &mut Vec<Log>,
        include_zksync: bool,
        include_erc: bool,
        token_addresses: Option<&HashSet<H160>>,
    ) -> Result<()> {
        let common_data = CommonLogData {
            block_hash: Some(H256::from_slice(&receipt.block_hash)),
            block_number: Some(receipt.block_number.into()),
            transaction_hash: H256::from_slice(&receipt.tx_hash),
            // U64::MAX for failed transactions
            transaction_index: Some(receipt.block_index.map(Into::into).unwrap_or(U64::MAX)),
        };
        let op: Option<ZkSyncOp> = serde_json::from_value(receipt.operation).unwrap();
        if let Some(op) = op {
            if include_erc {
                let erc_logs = self
                    .logs_helper
                    .erc_logs(op.clone(), common_data.clone(), storage)
                    .await?;
                for log in erc_logs {
                    if let Some(token_addresses) = token_addresses {
                        // We need to filter it here because of swaps.
                        // They produce 2 erc transfers and it can be
                        // that only one of them satisfies filter.
                        if token_addresses.contains(&log.address) {
                            logs.push(log);
                        }
                    } else {
                        logs.push(log);
                    }
                }
            }
            if include_zksync {
                let zksync_log = self
                    .logs_helper
                    .zksync_log(op, common_data, storage)
                    .await?;
                if let Some(zksync_log) = zksync_log {
                    logs.push(zksync_log);
                }
            }
        }
        Ok(())
    }

    async fn tx_receipt(
        &self,
        storage: &mut StorageProcessor<'_>,
        receipt: Web3TxReceipt,
    ) -> Result<TransactionReceipt> {
        let mut logs = Vec::new();
        self.append_logs(storage, receipt.clone(), &mut logs, true, true, None)
            .await?;
        let root_hash = H256::from_slice(&receipt.block_hash);
        Ok(TransactionReceipt {
            transaction_hash: H256::from_slice(&receipt.tx_hash),
            // U64::MAX for failed transactions
            transaction_index: receipt.block_index.map(Into::into).unwrap_or(U64::MAX),
            block_hash: Some(root_hash),
            block_number: Some(receipt.block_number.into()),
            cumulative_gas_used: 0.into(),
            gas_used: Some(0.into()),
            contract_address: None,
            logs,
            status: Some((receipt.success as u8).into()),
            root: Some(root_hash),
            logs_bloom: H2048::zero(),
        })
    }

    async fn block_by_number(
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
            let parent_block = transaction
                .chain()
                .block_schema()
                .get_storage_block(block_number - 1)
                .await
                .map_err(|_| Error::internal_error())?
                .expect("Can't find parent block in storage");
            H256::from_slice(&parent_block.root_hash)
        };

        if include_txs {
            // It was already checked that the block is in storage.
            let block = transaction
                .chain()
                .block_schema()
                .get_block(block_number)
                .await
                .map_err(|_| Error::internal_error())?
                .expect("Can't find block in storage");
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
                            to: op.priority_op.data.to_account(),
                            nonce: op.priority_op.serial_id as u32,
                            tx_hash: H256::from_slice(op.priority_op.tx_hash().as_ref()),
                        },
                    };
                    transaction_from_tx_data(tx)
                })
                .collect();

            Ok(BlockInfo::new_with_txs(
                hash,
                parent_hash,
                block_number,
                block.timestamp,
                transactions,
            ))
        } else {
            // It was already checked that the block is in storage.
            let block = transaction
                .chain()
                .block_schema()
                .get_storage_block(block_number)
                .await
                .map_err(|_| Error::internal_error())?
                .expect("Can't find block in storage");
            let hash = H256::from_slice(&block.root_hash);
            let transactions = transaction
                .chain()
                .block_schema()
                .get_block_transactions_hashes(block_number)
                .await
                .map_err(|_| Error::internal_error())?
                .into_iter()
                .map(|hash| H256::from_slice(&hash))
                .collect();

            transaction
                .commit()
                .await
                .map_err(|_| Error::internal_error())?;

            Ok(BlockInfo::new_with_hashes(
                hash,
                parent_hash,
                block_number,
                block.timestamp.unwrap_or_default() as u64,
                transactions,
            ))
        }
    }

    fn process_topics(&self, topics: Vec<H256>) -> (bool, Vec<String>) {
        let mut has_erc_topic = false;
        let mut tx_types = Vec::new();
        for topic in topics {
            if let Some(event) = self.logs_helper.event_by_topic(&topic) {
                if matches!(event, Event::ERCTransfer) {
                    has_erc_topic = true;
                } else {
                    tx_types.push(format!("{:?}", event).replace("ZkSync", ""));
                }
            }
        }
        (has_erc_topic, tx_types)
    }

    fn process_addresses(&self, addresses: Vec<H160>) -> (bool, Vec<H160>) {
        let mut has_zksync_proxy_address = false;
        let mut token_addresses = Vec::new();
        for address in addresses {
            if address == self.logs_helper.zksync_proxy_address {
                has_zksync_proxy_address = true;
            } else {
                token_addresses.push(address);
            }
        }
        (has_zksync_proxy_address, token_addresses)
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
