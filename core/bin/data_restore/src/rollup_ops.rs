use web3::{Transport, Web3};

use zksync_types::operations::ZkSyncOp;

use crate::contract::ZkSyncContractVersion;
use crate::eth_tx_helpers::{get_ethereum_transaction, get_input_data_from_ethereum_transaction};
use crate::events::BlockEvent;
use zksync_types::{AccountId, BlockNumber, H256};

/// Description of a Rollup operations block
#[derive(Debug, Clone)]
pub struct RollupOpsBlock {
    /// Rollup block number
    pub block_num: BlockNumber,
    /// Rollup operations in block
    pub ops: Vec<ZkSyncOp>,
    /// Fee account
    pub fee_account: AccountId,
    /// Timestamp
    pub timestamp: Option<u64>,
    /// Previous block root hash.
    pub previous_block_root_hash: H256,
    /// zkSync contract version for the given block.
    /// Used to obtain block chunk sizes. Stored in the database
    /// in the corresponding block event.
    pub contract_version: Option<ZkSyncContractVersion>,
}

impl RollupOpsBlock {
    /// Returns a Rollup operations block description
    ///
    /// # Arguments
    ///
    /// * `web3` - Web3 provider url
    /// * `event_data` - Rollup contract event description
    ///
    ///
    pub async fn get_rollup_ops_blocks<T: Transport>(
        web3: &Web3<T>,
        event_data: &BlockEvent,
    ) -> anyhow::Result<Vec<Self>> {
        let transaction = get_ethereum_transaction(web3, &event_data.transaction_hash).await?;
        let input_data = get_input_data_from_ethereum_transaction(&transaction)?;
        let blocks: Vec<RollupOpsBlock> = event_data
            .contract_version
            .rollup_ops_blocks_from_bytes(input_data)?;
        Ok(blocks)
    }
}
