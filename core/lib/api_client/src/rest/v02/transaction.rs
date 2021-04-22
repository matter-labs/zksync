use crate::rest::client::{Client, Result};
use zksync_api_types::v02::{
    transaction::{IncomingTx, IncomingTxBatch},
    Response,
};
use zksync_types::tx::{EthBatchSignatures, TxEthSignature, TxHash, ZkSyncTx};

impl Client {
    pub async fn submit_tx_v02(
        &self,
        tx: ZkSyncTx,
        signature: Option<TxEthSignature>,
    ) -> Result<Response> {
        self.post_with_scope(super::API_V02_SCOPE, "transaction")
            .body(&IncomingTx { tx, signature })
            .send()
            .await
    }

    pub async fn submit_batch_v02(
        &self,
        txs: Vec<ZkSyncTx>,
        signature: EthBatchSignatures,
    ) -> Result<Response> {
        self.post_with_scope(super::API_V02_SCOPE, "transaction/batches")
            .body(&IncomingTxBatch { txs, signature })
            .send()
            .await
    }

    pub async fn tx_status_v02(&self, tx_hash: TxHash) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("transaction/{}", tx_hash.to_string()),
        )
        .send()
        .await
    }

    pub async fn tx_data_v02(&self, tx_hash: TxHash) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("transaction/{}/data", tx_hash.to_string()),
        )
        .send()
        .await
    }

    pub async fn get_batch(&self, batch_hash: TxHash) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("transaction/batches/{}", batch_hash.to_string()),
        )
        .send()
        .await
    }
}
