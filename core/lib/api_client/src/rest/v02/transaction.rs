use crate::rest::client::{Client, Result};
use zksync_api_types::{
    v02::{transaction::IncomingTxBatch, Response},
    TxWithSignature,
};
use zksync_types::tx::{EthBatchSignatures, TxEthSignatureVariant, TxHash, ZkSyncTx};

impl Client {
    pub async fn submit_tx(
        &self,
        tx: ZkSyncTx,
        signature: TxEthSignatureVariant,
    ) -> Result<Response> {
        self.post_with_scope(super::API_V02_SCOPE, "transactions")
            .body(&TxWithSignature { tx, signature })
            .send()
            .await
    }

    pub async fn submit_batch(
        &self,
        txs: Vec<TxWithSignature>,
        signature: Option<EthBatchSignatures>,
    ) -> Result<Response> {
        self.post_with_scope(super::API_V02_SCOPE, "transactions/batches")
            .body(&IncomingTxBatch { txs, signature })
            .send()
            .await
    }

    pub async fn tx_status(&self, tx_hash: TxHash) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("transactions/{}", tx_hash.to_string()),
        )
        .send()
        .await
    }

    pub async fn tx_data(&self, tx_hash: TxHash) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("transactions/{}/data", tx_hash.to_string()),
        )
        .send()
        .await
    }

    pub async fn get_batch(&self, batch_hash: TxHash) -> Result<Response> {
        self.get_with_scope(
            super::API_V02_SCOPE,
            &format!("transactions/batches/{}", batch_hash.to_string()),
        )
        .send()
        .await
    }
}
