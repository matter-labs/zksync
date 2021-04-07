// Local uses
use crate::rest::client::{Client, Result};
use zksync_types::{
    api_v02::{
        fee::{BatchFeeRequest, TxFeeRequest, TxInBatchFeeRequest},
        Response,
    },
    Address, TokenLike, TxFeeTypes,
};

/// Block API part.
impl Client {
    /// Get fee for single transaction.
    pub async fn get_txs_fee_v02(
        &self,
        tx_type: TxFeeTypes,
        address: Address,
        token_like: TokenLike,
    ) -> Result<Response> {
        self.post_with_scope(super::API_V02_SCOPE, "fee")
            .body(&TxFeeRequest {
                tx_type,
                address,
                token_like,
            })
            .send()
            .await
    }

    /// Get txs fee for batch.
    pub async fn get_batch_fee_v02(
        &self,
        transactions: Vec<TxInBatchFeeRequest>,
        token_like: TokenLike,
    ) -> Result<Response> {
        self.post_with_scope(super::API_V02_SCOPE, "fee/batch")
            .body(&BatchFeeRequest {
                transactions,
                token_like,
            })
            .send()
            .await
    }
}
