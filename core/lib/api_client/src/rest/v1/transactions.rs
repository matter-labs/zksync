//! Transactions part of API implementation.

// Built-in uses

// External uses
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync_types::{
    tx::{EthBatchSignatures, EthSignData, TxEthSignatureVariant, TxHash},
    Address, BatchFee, BlockNumber, Fee, SignedZkSyncTx, TokenLike, TxFeeTypes, ZkSyncTx,
};

// Local uses
use super::{client::Client, client::ClientError, Pagination};

// Data transfer objects.

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub struct FastProcessingQuery {
    pub fast_processing: Option<bool>,
}

/// This structure has the same layout as [`SignedZkSyncTx`],
/// the only difference is that it uses "camelCase" for serialization.
///
/// [`SignedZkSyncTx`]: zksync_types::SignedZkSyncTx
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TxData {
    /// Underlying zkSync transaction.
    pub tx: ZkSyncTx,
    /// Tuple of the Ethereum signature and the message
    /// which user should have signed with their private key.
    /// Can be `None` if the Ethereum signature is not required.
    pub eth_sign_data: Option<EthSignData>,
}

/// This struct has the same layout as `SignedZkSyncTx`, expect that it used
/// `TxEthSignature` directly instead of `EthSignData`.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IncomingTx {
    pub tx: ZkSyncTx,
    pub signature: TxEthSignatureVariant,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IncomingTxForFee {
    pub tx_type: TxFeeTypes,
    pub address: Address,
    pub token_like: TokenLike,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IncomingTxBatchForFee {
    pub tx_types: Vec<TxFeeTypes>,
    pub addresses: Vec<Address>,
    pub token_like: TokenLike,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct IncomingTxBatch {
    pub txs: Vec<ZkSyncTx>,
    pub signature: EthBatchSignatures,
}

/// Transaction (or priority operation) receipt.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum Receipt {
    /// The transaction is awaiting execution in the memorypool.
    Pending,
    /// The transaction has been executed, but the block containing this transaction has not
    /// yet been committed.
    Executed,
    /// The block which contains this transaction has been committed.
    Committed { block: BlockNumber },
    /// The block which contains this transaction has been verified.
    Verified { block: BlockNumber },
    /// The transaction has been rejected for some reasons.
    Rejected { reason: Option<String> },
}

impl From<TxData> for SignedZkSyncTx {
    fn from(inner: TxData) -> Self {
        Self {
            tx: inner.tx,
            eth_sign_data: inner.eth_sign_data,
        }
    }
}

impl From<SignedZkSyncTx> for TxData {
    fn from(inner: SignedZkSyncTx) -> Self {
        Self {
            tx: inner.tx,
            eth_sign_data: inner.eth_sign_data,
        }
    }
}

/// Transactions API part.
impl Client {
    /// Sends a new transaction to the memory pool.
    pub async fn submit_tx(
        &self,
        tx: ZkSyncTx,
        signature: TxEthSignatureVariant,
        fast_processing: Option<bool>,
    ) -> Result<TxHash, ClientError> {
        self.post("transactions/submit")
            .query(&FastProcessingQuery { fast_processing })
            .body(&IncomingTx { tx, signature })
            .send()
            .await
    }

    /// Get fee for single transaction.
    pub async fn get_txs_fee(
        &self,
        tx_type: TxFeeTypes,
        address: Address,
        token_like: TokenLike,
    ) -> Result<Fee, ClientError> {
        self.post("transactions/fee")
            .body(&IncomingTxForFee {
                tx_type,
                address,
                token_like,
            })
            .send()
            .await
    }

    /// Get txs fee for batch.
    pub async fn get_batched_txs_fee(
        &self,
        tx_types: Vec<TxFeeTypes>,
        addresses: Vec<Address>,
        token_like: TokenLike,
    ) -> Result<BatchFee, ClientError> {
        self.post("transactions/fee/batch")
            .body(&IncomingTxBatchForFee {
                tx_types,
                addresses,
                token_like,
            })
            .send()
            .await
    }

    /// Sends a new transactions batch to the memory pool.
    pub async fn submit_tx_batch(
        &self,
        txs: Vec<ZkSyncTx>,
        signature: EthBatchSignatures,
    ) -> Result<Vec<TxHash>, ClientError> {
        self.post("transactions/submit/batch")
            .body(&IncomingTxBatch { txs, signature })
            .send()
            .await
    }

    /// Gets actual transaction receipt.
    pub async fn tx_status(&self, tx_hash: TxHash) -> Result<Option<Receipt>, ClientError> {
        self.get(&format!("transactions/{}", tx_hash.to_string()))
            .send()
            .await
    }

    /// Gets transaction content.
    pub async fn tx_data(&self, tx_hash: TxHash) -> Result<Option<TxData>, ClientError> {
        self.get(&format!("transactions/{}/data", tx_hash.to_string()))
            .send()
            .await
    }

    /// Gets transaction receipt by ID.
    pub async fn tx_receipt_by_id(
        &self,
        tx_hash: TxHash,
        receipt_id: u32,
    ) -> Result<Option<Receipt>, ClientError> {
        self.get(&format!(
            "transactions/{}/receipts/{}",
            tx_hash.to_string(),
            receipt_id
        ))
        .send()
        .await
    }

    /// Gets transaction receipts.
    pub async fn tx_receipts(
        &self,
        tx_hash: TxHash,
        from: Pagination,
        limit: u32,
    ) -> Result<Vec<Receipt>, ClientError> {
        self.get(&format!("transactions/{}/receipts", tx_hash.to_string()))
            .query(&from.into_query(limit))
            .send()
            .await
    }
}
