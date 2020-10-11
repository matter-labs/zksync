use super::{api_decl::ApiV01, network_status::NetworkStatus, types::*};
use actix_web::Result as ActixResult;
use zksync_types::Token;

impl ApiV01 {
    pub async fn testnet_config(&self) -> ActixResult<TestnetConfigResponse> {
        let contract_address = self.contract_address.clone();
        Ok(TestnetConfigResponse { contract_address })
    }

    pub async fn status(&self) -> ActixResult<NetworkStatus> {
        Ok(self.network_status.read())
    }

    pub async fn tokens(&self) -> ActixResult<Vec<Token>> {
        let mut storage = self.access_storage().await?;
        let tokens = storage
            .tokens_schema()
            .load_tokens()
            .await
            .map_err(Self::db_error)?;

        let mut vec_tokens = tokens.values().cloned().collect::<Vec<_>>();
        vec_tokens.sort_by_key(|t| t.id);

        Ok(vec_tokens)
    }

    // fn tx_history(&self, address: Address, offset: u64, limit: u32) -> !;
    // fn tx_history_older_than(&self, address: Address, query: TxHistoryQuery) -> !;
    // fn tx_history_newer_than(&self, address: Address, query: TxHistoryQuery) -> !;
    // fn executed_tx_by_hash(&self, hash: H256) -> !;
    // fn tx_by_hash(&self, hash: H256) -> !;
    // fn priority_op(&self, pq_id: u64) -> !;
    // fn block_tx(&self, block_id: BlockNumber, tx_id: u32) -> !;
    // fn block_tx(&self, block_id: BlockNumber, tx_id: u32) -> !;
    // fn block_transactions(&self, block_id: BlockNumber) -> !;
    // fn blocks(&self, block_query: HandleBlocksQuery) -> !;
    // fn explorer_search(&self, block_query: BlockExplorerSearchQuery) -> !;
    // fn withdrawal_processing_time(&self) -> WithdrawalProcessingTimeResponse;
}
