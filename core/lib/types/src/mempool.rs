use super::{
    tx::{TxEthSignature, TxHash},
    SerialId, SignedZkSyncTx,
};

/// A collection of transactions that must be executed together.
/// All the transactions in the batch must be included into the same block,
/// and either succeed or fail all together.
#[derive(Debug, Clone)]
pub struct SignedTxsBatch {
    pub txs: Vec<SignedZkSyncTx>,
    pub batch_id: i64,
    pub eth_signatures: Vec<TxEthSignature>,
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
    pub fn batch(
        txs: Vec<SignedZkSyncTx>,
        batch_id: i64,
        eth_signatures: Vec<TxEthSignature>,
    ) -> Self {
        Self::Batch(SignedTxsBatch {
            txs,
            batch_id,
            eth_signatures,
        })
    }

    pub fn hashes(&self) -> Vec<TxHash> {
        match self {
            Self::Tx(tx) => vec![tx.hash()],
            Self::Batch(batch) => batch.txs.iter().map(|tx| tx.hash()).collect(),
        }
    }

    pub fn get_transactions(&self) -> Vec<SignedZkSyncTx> {
        match self {
            Self::Tx(tx) => vec![tx.clone()],
            Self::Batch(batch) => batch.txs.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct RevertedTxVariant {
    inner: SignedTxVariant,
    pub next_priority_op_id: SerialId,
}

impl AsRef<SignedTxVariant> for RevertedTxVariant {
    fn as_ref(&self) -> &SignedTxVariant {
        &self.inner
    }
}

impl AsMut<SignedTxVariant> for RevertedTxVariant {
    fn as_mut(&mut self) -> &mut SignedTxVariant {
        &mut self.inner
    }
}

impl RevertedTxVariant {
    pub fn new(txs: SignedTxVariant, next_priority_op_id: SerialId) -> Self {
        Self {
            inner: txs,
            next_priority_op_id,
        }
    }

    pub fn into_inner(self) -> SignedTxVariant {
        self.inner
    }
}
