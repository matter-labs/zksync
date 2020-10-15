use super::{tx::TxHash, SignedZkSyncTx};

/// A collection of transactions that must be executed together.
/// All the transactions in the batch must be included into the same block,
/// and either succeed or fail all together.
#[derive(Debug, Clone)]
pub struct SignedTxsBatch {
    pub txs: Vec<SignedZkSyncTx>,
    pub batch_id: i64,
}

/// A wrapper around possible atomic block elements: it can be either
/// a single transaction, or the transactions batch.
#[derive(Debug, Clone)]
pub enum SignedTxVariant {
    Tx(SignedZkSyncTx),
    Batch(SignedTxsBatch),
}

impl From<SignedZkSyncTx> for SignedTxVariant {
    fn from(tx: SignedZkSyncTx) -> Self {
        Self::Tx(tx)
    }
}

impl SignedTxVariant {
    pub fn batch(txs: Vec<SignedZkSyncTx>, batch_id: i64) -> Self {
        Self::Batch(SignedTxsBatch { txs, batch_id })
    }

    pub fn hashes(&self) -> Vec<TxHash> {
        match self {
            Self::Tx(tx) => vec![tx.hash()],
            Self::Batch(batch) => batch.txs.iter().map(|tx| tx.hash()).collect(),
        }
    }
}
