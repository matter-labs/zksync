use crate::tx_error::TxAddError;
use zksync_types::{Address, PriorityOp, SignedZkSyncTx, H256};

#[derive(Debug, Clone)]
pub struct CoreApiClient {
    client: reqwest::Client,
    addr: String,
}

pub type EthBlockId = u64;

impl CoreApiClient {
    pub fn new(addr: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            addr,
        }
    }

    pub async fn send_tx(&self, _tx: SignedZkSyncTx) -> anyhow::Result<Result<(), TxAddError>> {
        // TODO
        Ok(Ok(()))
    }

    pub async fn send_txs_batch(
        &self,
        _txs: Vec<SignedZkSyncTx>,
    ) -> anyhow::Result<Result<(), TxAddError>> {
        // TODO
        Ok(Ok(()))
    }

    pub async fn get_unconfirmed_deposits(
        &self,
        address: Address,
    ) -> anyhow::Result<Vec<(EthBlockId, PriorityOp)>> {
        // TODO
        Ok(Vec::new())
    }

    pub async fn get_unconfirmed_op(
        &self,
        eth_tx_hash: H256,
    ) -> anyhow::Result<Option<(EthBlockId, PriorityOp)>> {
        // TODO
        Ok(None)
    }
}
