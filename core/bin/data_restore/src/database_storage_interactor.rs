// Built-in deps
use std::str::FromStr;
// Workspace deps
use zksync_storage::{
    data_restore::records::{NewBlockEvent, NewRollupOpsBlock},
    StorageProcessor,
};
use zksync_types::withdrawals::{WithdrawalEvent, WithdrawalPendingEvent};
use zksync_types::{
    aggregated_operations::{BlocksCommitOperation, BlocksExecuteOperation},
    AccountId, BlockNumber, NewTokenEvent, PriorityOp, SerialId, Token, TokenId, TokenInfo,
    TokenKind,
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
        stored_ops_block_into_ops_block, CachedTreeState,
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

    pub async fn start_transaction<'c: 'b, 'b>(&'c mut self) -> DatabaseStorageInteractor<'b> {
        let transaction = self
            .storage
            .start_transaction()
            .await
            .expect("Failed to start database transaction");
        DatabaseStorageInteractor {
            storage: transaction,
        }
    }

    pub async fn commit(self) {
        // Will panic if not in transaction.
        self.storage.commit().await.unwrap();
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

    pub async fn save_rollup_ops(&mut self, blocks: &[RollupOpsBlock]) {
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

    pub async fn update_tree_state(&mut self, block: Block, accounts_updated: AccountUpdates) {
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
            .save_full_block(block)
            .await
            .expect("Unable to save block");

        transaction
            .commit()
            .await
            .expect("Unable to commit DB transaction");
    }

    pub async fn apply_priority_op_data(
        &mut self,
        priority_op_data: impl Iterator<Item = &PriorityOp>,
    ) -> Vec<SerialId> {
        self.storage
            .data_restore_schema()
            .update_executed_priority_operations(priority_op_data)
            .await
            .expect("Failed to update executed priority operations")
    }

    pub async fn store_token(&mut self, token: TokenInfo, token_id: TokenId) {
        self.storage
            .tokens_schema()
            .store_token(Token::new(
                token_id,
                token.address,
                &token.symbol,
                token.decimals,
                TokenKind::ERC20,
            ))
            .await
            .expect("failed to store token");
    }

    pub async fn save_withdrawals(
        &mut self,
        withdrawals: &[WithdrawalEvent],
        pending_withdrawals: &[WithdrawalPendingEvent],
    ) {
        let mut transaction = self.storage.start_transaction().await.unwrap();
        transaction
            .withdrawals_schema()
            .save_pending_withdrawals(pending_withdrawals)
            .await
            .unwrap();
        for withdrawal in withdrawals {
            transaction
                .withdrawals_schema()
                .finalize_withdrawal(withdrawal)
                .await
                .unwrap();
        }
        transaction.commit().await.unwrap();
    }

    pub async fn save_events_state(
        &mut self,
        block_events: &[BlockEvent],
        tokens: &[NewTokenEvent],
        priority_op_data: &[PriorityOp],
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
            .save_events_state(
                new_events.as_slice(),
                &tokens,
                priority_op_data,
                &block_number,
            )
            .await
            .expect("Cant update events state");
    }

    pub async fn save_genesis_tree_state(
        &mut self,
        genesis_updates: &[(AccountId, AccountUpdate)],
    ) {
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

    pub async fn save_special_token(&mut self, token: Token) {
        self.storage
            .tokens_schema()
            .store_token(token)
            .await
            .expect("failed to store special token");
    }

    pub async fn get_block_events_state_from_storage(&mut self) -> EventsState {
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

        let priority_op_data = self
            .storage
            .data_restore_schema()
            .get_priority_op_data()
            .await
            .expect("Failed to load priority operations data");

        EventsState {
            committed_events,
            verified_events,
            last_watched_eth_block_number,
            priority_op_data,
        }
    }

    pub async fn get_tree_state(&mut self) -> StoredTreeState {
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

    pub async fn get_ops_blocks_from_storage(&mut self) -> Vec<RollupOpsBlock> {
        self.storage
            .data_restore_schema()
            .load_rollup_ops_blocks()
            .await
            .expect("Cant load operation blocks")
            .into_iter()
            .map(stored_ops_block_into_ops_block)
            .collect()
    }

    pub async fn update_eth_state(&mut self) {
        let mut transaction = self
            .storage
            .start_transaction()
            .await
            .expect("Failed to start database transaction");
        let last_committed_block = transaction
            .chain()
            .block_schema()
            .get_last_committed_block()
            .await
            .expect("Can't get the last committed block");

        let last_verified_block = transaction
            .chain()
            .block_schema()
            .get_last_verified_block()
            .await
            .expect("Can't get the last verified block");

        // Use new schema to get `last_committed`, `last_verified_block` and `last_executed_block` (ZKS-427).
        transaction
            .data_restore_schema()
            .initialize_eth_stats(
                last_committed_block,
                last_verified_block,
                last_verified_block,
            )
            .await
            .expect("Can't update the eth_stats table");
        transaction
            .commit()
            .await
            .expect("Failed to commit database transaction");
    }

    pub async fn get_cached_tree_state(&mut self) -> Option<CachedTreeState> {
        let (last_block, account_map) = self
            .storage
            .chain()
            .state_schema()
            .load_verified_state()
            .await
            .expect("Failed to load verified state from the database");

        let tree_cache = self
            .storage
            .chain()
            .tree_cache_schema_json()
            .get_account_tree_cache_block(last_block)
            .await
            .expect("Failed to query the database for the tree cache");

        if let Some(tree_cache) = tree_cache {
            let current_block = self
                .storage
                .chain()
                .block_schema()
                .get_block(last_block)
                .await
                .expect("Failed to query the database for the latest block")
                .unwrap();
            let nfts = self
                .storage
                .tokens_schema()
                .load_nfts()
                .await
                .expect("Failed to load NFTs from the database");
            Some(CachedTreeState {
                tree_cache,
                account_map,
                current_block,
                nfts,
            })
        } else {
            None
        }
    }

    pub async fn update_tree_cache(&mut self, block_number: BlockNumber, tree_cache: String) {
        let mut transaction = self
            .storage
            .start_transaction()
            .await
            .expect("Failed to start transaction");

        transaction
            .chain()
            .tree_cache_schema_json()
            .remove_old_account_tree_cache(block_number)
            .await
            .expect("Failed to remove old tree cache");

        // It is safe to store the new tree cache without additional checks
        // since on conflict it does nothing.
        transaction
            .chain()
            .tree_cache_schema_json()
            .store_account_tree_cache(block_number, tree_cache)
            .await
            .expect("Failed to store new tree cache");

        transaction
            .commit()
            .await
            .expect("Failed to update tree cache");
    }

    pub async fn get_storage_state(&mut self) -> StorageUpdateState {
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

    pub async fn get_max_priority_op_serial_id(&mut self) -> SerialId {
        self.storage
            .chain()
            .operations_schema()
            .get_max_priority_op_serial_id()
            .await
            .expect("Failed to retrieve maximum priority op serial id")
            .unwrap_or(0)
    }
}
