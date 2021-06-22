// Local uses
use crate::rest::client::{Client, Result};
use zksync_api_types::v02::{
    fee::{ApiTxFeeTypes, BatchFeeRequest, TxFeeRequest, TxInBatchFeeRequest},
    Response,
};
use zksync_types::{Address, TokenLike};

impl Client {
    pub async fn get_txs_fee(
        &self,
        tx_type: ApiTxFeeTypes,
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

    pub async fn get_batch_fee(
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
