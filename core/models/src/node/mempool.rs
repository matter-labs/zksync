use super::{tx::TxHash, SignedFranklinTx};

/// A collection of transactions that must be executed together.
/// All the transactions in the batch must be included into the same block,
/// and either succeed or fail all together.
#[derive(Debug, Clone)]
pub struct SignedTxsBatch(pub Vec<SignedFranklinTx>);

/// A wrapper around possible atomic block elements: it can be either
/// a single transaction, or the transactions batch.
#[derive(Debug, Clone)]
pub enum SignedTxVariant {
    Tx(SignedFranklinTx),
    Batch(SignedTxsBatch),
}

impl From<SignedFranklinTx> for SignedTxVariant {
    fn from(tx: SignedFranklinTx) -> Self {
        Self::Tx(tx)
    }
}

impl From<Vec<SignedFranklinTx>> for SignedTxVariant {
    fn from(txs: Vec<SignedFranklinTx>) -> Self {
        let batch = SignedTxsBatch(txs);
        Self::Batch(batch)
    }
}

impl SignedTxVariant {
    pub fn hashes(&self) -> Vec<TxHash> {
        match self {
            Self::Tx(tx) => vec![tx.hash()],
            Self::Batch(batch) => batch.0.iter().map(|tx| tx.hash()).collect(),
        }
    }
}
