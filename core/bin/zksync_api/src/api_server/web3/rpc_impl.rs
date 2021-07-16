// Built-in uses
use std::time::Instant;
// External uses
use jsonrpc_core::{Error, Result};
// Workspace uses
use zksync_crypto::convert::FeConvert;
use zksync_storage::StorageProcessor;
use zksync_types::ExecutedOperations;
// Local uses
use super::{
    converter::{
        resolve_block_number, transaction_from_tx_data, tx_receipt_from_storage_receipt,
        u256_from_biguint,
    },
    types::{BlockInfo, BlockNumber, Transaction, TransactionReceipt, TxData, H256, U256, U64},
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
        address: zksync_types::Address,
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
            .tx_receipt_for_web3(hash.as_ref())
            .await
            .map_err(|_| Error::internal_error())?;
        let result = tx.map(tx_receipt_from_storage_receipt);

        metrics::histogram!("api.web3.get_transaction_receipt", start.elapsed());
        Ok(result)
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
                            block_hash: Some(hash),
                            block_number: Some(block_number.0),
                            block_index: tx.block_index,
                            from: tx.signed_tx.tx.from_account(),
                            to: tx.signed_tx.tx.to_account(),
                            nonce: tx.signed_tx.tx.nonce().0,
                            tx_hash: H256::from_slice(tx.signed_tx.tx.hash().as_ref()),
                        },
                        ExecutedOperations::PriorityOp(op) => TxData {
                            block_hash: Some(hash),
                            block_number: Some(block_number.0),
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
}
