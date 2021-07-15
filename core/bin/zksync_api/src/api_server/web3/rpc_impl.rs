// Built-in uses
use std::time::Instant;
// External uses
use jsonrpc_core::{Error, Result};
// Workspace uses
// Local uses
use super::{
    types::{
        tx_receipt_from_storage_receipt, BlockInfo, BlockNumber, Transaction, TransactionReceipt,
        H256, U256, U64,
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

        let tx = storage
            .chain()
            .operations_ext_schema()
            .tx_data_for_web3(hash.as_ref())
            .await
            .map_err(|_| Error::internal_error())?;
        let result = tx.map(|tx| Self::transaction_from_tx_data(tx.into()));

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
}
