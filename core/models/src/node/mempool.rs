use super::tx::TxHash;
use crate::node::SignedFranklinTx;

/// A collection of transactions that must be executed together.
/// All the transactions in the batch must be included into the same block,
/// and either succeed or fail all together.
#[derive(Debug, Clone)]
pub struct TxsBatch(pub Vec<SignedFranklinTx>);

/// A wrapper around possible atomic block elements: it can be either
/// a single transaction, or the transactions batch.
#[derive(Debug, Clone)]
pub enum TxVariant {
    Tx(SignedFranklinTx),
    Batch(TxsBatch),
}

impl From<SignedFranklinTx> for TxVariant {
    fn from(tx: SignedFranklinTx) -> Self {
        Self::Tx(tx)
    }
}

impl From<Vec<SignedFranklinTx>> for TxVariant {
    fn from(txs: Vec<SignedFranklinTx>) -> Self {
        let batch = TxsBatch(txs);
        Self::Batch(batch)
    }
}

impl TxVariant {
    pub fn hashes(&self) -> Vec<TxHash> {
        match self {
            Self::Tx(tx) => vec![tx.hash()],
            Self::Batch(batch) => batch.0.iter().map(|tx| tx.hash()).collect(),
        }
    }
}
