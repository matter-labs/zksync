// Built-in deps
use std::str::FromStr;
// Workspace deps
use zksync_storage::{
    data_restore::records::{NewBlockEvent, NewRollupOpsBlock},
    StorageProcessor,
};
use zksync_types::{
    aggregated_operations::{BlocksCommitOperation, BlocksExecuteOperation},
    AccountId, NewTokenEvent, Token, TokenId, TokenInfo,
    {block::Block, AccountUpdate, AccountUpdates},
};

// Local deps
use crate::storage_interactor::StoredTreeState;
use crate::{
    data_restore_driver::StorageUpdateState,
    events::BlockEvent,
    events_state::EventsState,
    rollup_ops::RollupOpsBlock,
    storage_interactor::{
        block_event_into_stored_block_event, stored_block_event_into_block_event,
        stored_ops_block_into_ops_block, StorageInteractor,
    },
};

pub struct DatabaseStorageInteractor<'a> {
    storage: StorageProcessor<'a>,
}

impl<'a> DatabaseStorageInteractor<'a> {
    pub fn new(storage: StorageProcessor<'a>) -> Self {
        Self { storage }
    }

    pub fn storage(&mut self) -> &mut StorageProcessor<'a> {
        &mut self.storage
    }

    /// Returns last watched ethereum block number from storage
    pub async fn get_last_watched_block_number_from_storage(&mut self) -> u64 {
        let last_watched_block_number_string = self
            .storage
            .data_restore_schema()
            .load_last_watched_block_number()
            .await
            .expect("Cant load last watched block number")
            .block_number;

        u64::from_str(last_watched_block_number_string.as_str())
            .expect("Сant make u256 block_number in get_last_watched_block_number_from_storage")
    }
}

#[async_trait::async_trait]
impl StorageInteractor for DatabaseStorageInteractor<'_> {
    async fn save_rollup_ops(&mut self, blocks: &[RollupOpsBlock]) {
        let mut ops = Vec::with_capacity(blocks.len());

        for block in blocks {
            ops.push(NewRollupOpsBlock {
                block_num: block.block_num,
                ops: block.ops.as_slice(),
                fee_account: block.fee_account,
                timestamp: block.timestamp,
                previous_block_root_hash: block.previous_block_root_hash,
            });
        }

        self.storage
            .data_restore_schema()
            .save_rollup_ops(ops.as_slice())
            .await
            .expect("Cant update rollup operations");
    }

    async fn update_tree_state(&mut self, block: Block, accounts_updated: AccountUpdates) {
        let mut transaction = self
            .storage
            .start_transaction()
            .await
            .expect("Failed initializing a DB transaction");

        let commit_aggregated_operation = BlocksCommitOperation {
            last_committed_block: block.clone(),
            blocks: vec![block.clone()],
        };

        let execute_aggregated_operation = BlocksExecuteOperation {
            blocks: vec![block.clone()],
        };

        transaction
            .chain()
            .state_schema()
            .commit_state_update(block.block_number, &accounts_updated, 0)
            .await
            .expect("Cant execute verify operation");

        transaction
            .data_restore_schema()
            .save_block_operations(commit_aggregated_operation, execute_aggregated_operation)
            .await
            .expect("Cant execute verify operation");

        transaction
            .chain()
            .block_schema()
            .save_block(block)
            .await
            .expect("Unable to save block");

        transaction
            .commit()
            .await
            .expect("Unable to commit DB transaction");
    }

    async fn store_token(&mut self, token: TokenInfo, token_id: TokenId) {
        self.storage
            .tokens_schema()
            .store_token(Token {
                id: token_id,
                symbol: token.symbol,
                address: token.address,
                decimals: token.decimals,
                is_nft: false,
            })
            .await
            .expect("failed to store token");
    }

    async fn save_events_state(
        &mut self,
        block_events: &[BlockEvent],
        tokens: &[NewTokenEvent],
        last_watched_eth_block_number: u64,
    ) {
        let mut new_events: Vec<NewBlockEvent> = vec![];
        for event in block_events {
            new_events.push(block_event_into_stored_block_event(event));
        }

        let block_number = last_watched_eth_block_number.to_string();

        let tokens: Vec<_> = tokens
            .iter()
            .map(
                |event| zksync_storage::data_restore::records::NewTokenEvent {
                    address: event.address,
                    id: event.id,
                },
            )
            .collect();
        self.storage
            .data_restore_schema()
            .save_events_state(new_events.as_slice(), &tokens, &block_number)
            .await
            .expect("Cant update events state");
    }

    async fn save_genesis_tree_state(&mut self, genesis_updates: &[(AccountId, AccountUpdate)]) {
        let (_last_committed, mut _accounts) = self
            .storage
            .chain()
            .state_schema()
            .load_committed_state(None)
            .await
            .expect("Cant load comitted state");
        assert!(
            *_last_committed == 0 && _accounts.is_empty(),
            "db should be empty"
        );
        self.storage
            .data_restore_schema()
            .save_genesis_state(genesis_updates)
            .await
            .expect("Cant update genesis state");
    }

    async fn save_special_token(&mut self, token: Token) {
        self.storage
            .tokens_schema()
            .store_token(token)
            .await
            .expect("failed to store special token");
    }

    async fn get_block_events_state_from_storage(&mut self) -> EventsState {
        let last_watched_eth_block_number = self.get_last_watched_block_number_from_storage().await;

        let committed = self
            .storage
            .data_restore_schema()
            .load_committed_events_state()
            .await
            .expect("Cant load committed state");

        let mut committed_events: Vec<BlockEvent> = vec![];
        for event in committed {
            let block_event = stored_block_event_into_block_event(event.clone());
            committed_events.push(block_event);
        }

        let verified = self
            .storage
            .data_restore_schema()
            .load_verified_events_state()
            .await
            .expect("Cant load verified state");
        let mut verified_events: Vec<BlockEvent> = vec![];
        for event in verified {
            let block_event = stored_block_event_into_block_event(event.clone());
            verified_events.push(block_event);
        }

        EventsState {
            committed_events,
            verified_events,
            last_watched_eth_block_number,
        }
    }

    async fn get_tree_state(&mut self) -> StoredTreeState {
        let (last_block, account_map) = self
            .storage
            .chain()
            .state_schema()
            .load_verified_state()
            .await
            .expect("There are no last verified state in storage");

        let block = self
            .storage
            .chain()
            .block_schema()
            .get_block(last_block)
            .await
            .expect("Cant get the last block from storage")
            .expect("There are no last block in storage - restart driver");
        let (unprocessed_prior_ops, fee_acc_id) =
            (block.processed_priority_ops.1, block.fee_account);

        StoredTreeState {
            last_block_number: last_block,
            account_map,
            unprocessed_prior_ops,
            fee_acc_id,
        }
    }

    async fn get_ops_blocks_from_storage(&mut self) -> Vec<RollupOpsBlock> {
        self.storage
            .data_restore_schema()
            .load_rollup_ops_blocks()
            .await
            .expect("Cant load operation blocks")
            .into_iter()
            .map(stored_ops_block_into_ops_block)
            .collect()
    }

    async fn update_eth_state(&mut self) {
        let last_committed_block = self
            .storage
            .chain()
            .block_schema()
            .get_last_committed_block()
            .await
            .expect("Can't get the last committed block");

        let last_verified_block = self
            .storage
            .chain()
            .block_schema()
            .get_last_verified_block()
            .await
            .expect("Can't get the last verified block");

        // Use new schema to get `last_committed`, `last_verified_block` and `last_executed_block` (ZKS-427).
        self.storage
            .data_restore_schema()
            .initialize_eth_stats(
                last_committed_block,
                last_verified_block,
                last_verified_block,
            )
            .await
            .expect("Can't update the eth_stats table")
    }

    async fn get_storage_state(&mut self) -> StorageUpdateState {
        let storage_state_string = self
            .storage
            .data_restore_schema()
            .load_storage_state()
            .await
            .expect("Cant load storage state")
            .storage_state;

        match storage_state_string.as_ref() {
            "Events" => StorageUpdateState::Events,
            "Operations" => StorageUpdateState::Operations,
            "None" => StorageUpdateState::None,
            _ => panic!("Unknown storage state"),
        }
    }
}
