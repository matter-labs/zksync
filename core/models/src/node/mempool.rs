use super::{tx::TxHash, FranklinTx};

/// A collection of transactions that must be executed together.
/// All the transactions in the batch must be included into the same block,
/// and either succeed or fail all together.
#[derive(Debug, Clone)]
pub struct TxsBatch(pub Vec<FranklinTx>);

/// A wrapper around possible atomic block elements: it can be either
/// a single transaction, or the transactions batch.
#[derive(Debug, Clone)]
pub enum TxVariant {
    Tx(FranklinTx),
    Batch(TxsBatch),
}

impl From<FranklinTx> for TxVariant {
    fn from(tx: FranklinTx) -> Self {
        Self::Tx(tx)
    }
}

impl From<Vec<FranklinTx>> for TxVariant {
    fn from(txs: Vec<FranklinTx>) -> Self {
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
