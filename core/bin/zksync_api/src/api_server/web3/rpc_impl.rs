// Built-in uses
use std::time::Instant;
// External uses
use jsonrpc_core::{Error, Result};
// Workspace uses
// Local uses
use super::{
    types::{BlockInfo, BlockNumber, Transaction, TxData, H160, H256, U256, U64},
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
        let block_number = Self::resolve_block_number(&mut transaction, block)
            .await?
            .ok_or_else(|| Error::invalid_params("Block with such number doesn't exist yet"))?;
        let balance = transaction
            .chain()
            .account_schema()
            .get_account_eth_balance_for_block(address, block_number)
            .await
            .map_err(|_| Error::internal_error())?;
        let result =
            U256::from_dec_str(&balance.to_string()).map_err(|_| Error::internal_error())?;
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

        let block_number = Self::resolve_block_number(&mut transaction, block).await?;
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
        let mut transaction = storage
            .start_transaction()
            .await
            .map_err(|_| Error::internal_error())?;

        let executed_tx = if let Some(tx) = transaction
            .chain()
            .operations_schema()
            .get_executed_operation(hash.as_bytes())
            .await
            .map_err(|_| Error::internal_error())?
        {
            Some(TxData {
                block_number: tx.block_number as u32,
                block_index: tx.block_index.map(|i| i as u32),
                from: H160::from_slice(&tx.from_account),
                to: tx.to_account.map(|to| H160::from_slice(&to)),
                nonce: tx.nonce as u32,
                tx_hash: H256::from_slice(&tx.tx_hash),
            })
        } else if let Some(tx) = transaction
            .chain()
            .operations_schema()
            .get_executed_priority_operation_by_any_hash(hash.as_bytes())
            .await
            .map_err(|_| Error::internal_error())?
        {
            Some(TxData {
                block_number: tx.block_number as u32,
                block_index: Some(tx.block_index as u32),
                from: H160::from_slice(&tx.from_account),
                to: Some(H160::from_slice(&tx.to_account)),
                nonce: tx.priority_op_serialid as u32,
                tx_hash: H256::from_slice(&tx.tx_hash),
            })
        } else {
            None
        };

        let result = if let Some(tx) = executed_tx {
            let block = transaction
                .chain()
                .block_schema()
                .get_storage_block(zksync_types::BlockNumber(tx.block_number as u32))
                .await
                .map_err(|_| Error::internal_error())?
                .expect("Block of executed tx doesn't exist in storage");
            Some(Self::transaction_from_executed_tx_and_hash(
                tx,
                H256::from_slice(&block.root_hash),
            ))
        } else if let Some(tx) = transaction
            .chain()
            .mempool_schema()
            .get_tx(hash.as_bytes())
            .await
            .map_err(|_| Error::internal_error())?
        {
            Some(Transaction {
                hash: H256::from_slice(tx.tx.hash().as_ref()),
                nonce: tx.tx.nonce().0.into(),
                block_hash: None,
                block_number: None,
                transaction_index: None,
                from: tx.tx.from_account(),
                to: tx.tx.to_account(),
                value: 0.into(),
                gas_price: 0.into(),
                gas: 0.into(),
                input: Vec::new().into(),
                raw: None,
            })
        } else {
            None
        };

        transaction
            .commit()
            .await
            .map_err(|_| Error::internal_error())?;

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

        let block_number = Self::resolve_block_number(&mut transaction, block_number).await?;
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
}
