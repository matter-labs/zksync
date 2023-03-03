use std::cell::RefCell;
use std::cmp::max;
use std::collections::HashMap;
use std::sync::Arc;

use web3::types::Address;

use zksync_types::block::Block;
use zksync_types::withdrawals::{WithdrawalEvent, WithdrawalPendingEvent};
use zksync_types::{
    Account, AccountId, AccountMap, AccountUpdate, AccountUpdates, Action, BlockNumber,
    NewTokenEvent, Operation, PriorityOp, SerialId, Token, TokenId, TokenInfo, TokenKind,
};

use crate::{
    data_restore_driver::StorageUpdateState,
    events::{BlockEvent, EventType},
    events_state::EventsState,
    rollup_ops::RollupOpsBlock,
    storage_interactor::{CachedTreeState, StoredTreeState},
};

#[derive(Debug)]
struct Inner {
    rollups: Vec<RollupOpsBlock>,
    storage_state: StorageUpdateState,
    tokens: HashMap<TokenId, Token>,
    events_state: Vec<BlockEvent>,
    last_watched_block: u64,
    #[allow(dead_code)]
    last_committed_block: BlockNumber,
    last_verified_block: BlockNumber,
    accounts: AccountMap,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            rollups: vec![],
            storage_state: StorageUpdateState::None,
            tokens: Default::default(),
            events_state: vec![],
            last_watched_block: 0,
            last_committed_block: BlockNumber(0),
            last_verified_block: BlockNumber(0),
            accounts: Default::default(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct InMemoryStorageInteractor {
    inner: Arc<RefCell<Inner>>,
}

impl InMemoryStorageInteractor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_new_account(&mut self, id: AccountId, address: &Address) {
        let mut inner = self.inner.borrow_mut();

        inner
            .accounts
            .insert(id, Account::default_with_address(address));
    }

    pub fn get_account_by_address(&self, address: &Address) -> Option<(AccountId, Account)> {
        let inner = self.inner.borrow();
        let accounts: Vec<(AccountId, Account)> = inner
            .accounts
            .iter()
            .filter(|(_, acc)| acc.address == *address)
            .map(|(acc_id, acc)| (*acc_id, acc.clone()))
            .collect();
        accounts.first().cloned()
    }

    fn load_verified_events_state(&self) -> Vec<BlockEvent> {
        let inner = self.inner.borrow();
        inner
            .events_state
            .clone()
            .into_iter()
            .filter(|event| event.block_type == EventType::Verified)
            .collect()
    }

    pub async fn save_withdrawals(
        &mut self,
        _withdrawals: &[WithdrawalEvent],
        _pending_withdrawals: &[WithdrawalPendingEvent],
    ) {
        // We don't use it for testing right now
    }

    pub(crate) fn load_committed_events_state(&self) -> Vec<BlockEvent> {
        let inner = self.inner.borrow();
        // TODO avoid clone
        inner
            .events_state
            .clone()
            .into_iter()
            .filter(|event| event.block_type == EventType::Committed)
            .collect()
    }

    pub fn get_account(&self, id: &AccountId) -> Option<Account> {
        let inner = self.inner.borrow();
        inner.accounts.get(id).cloned()
    }

    fn commit_state_update(
        &mut self,
        first_update_order_id: u32,
        accounts_updated: AccountUpdates,
    ) {
        let mut inner = self.inner.borrow_mut();
        let update_order_ids =
            first_update_order_id..first_update_order_id + accounts_updated.len() as u32;

        for (_, (id, upd)) in update_order_ids.zip(accounts_updated.iter()) {
            match upd {
                AccountUpdate::Create { ref address, nonce } => {
                    let (mut acc, _) = Account::create_account(*id, *address);
                    acc.nonce = *nonce;
                    inner.accounts.insert(*id, acc);
                }
                AccountUpdate::Delete {
                    ref address,
                    nonce: _,
                } => {
                    let (acc_id, _) = self.get_account_by_address(address).unwrap();
                    inner.accounts.remove(&acc_id);
                }
                AccountUpdate::UpdateBalance {
                    balance_update: (token, _, new_balance),
                    old_nonce: _,
                    new_nonce,
                } => {
                    let account = inner
                        .accounts
                        .get_mut(id)
                        .expect("In tests this account should be stored");
                    account.set_balance(*token, new_balance.clone());
                    account.nonce = max(account.nonce, *new_nonce);
                }
                AccountUpdate::ChangePubKeyHash {
                    old_pub_key_hash: _,
                    ref new_pub_key_hash,
                    old_nonce: _,
                    new_nonce,
                } => {
                    let account = inner
                        .accounts
                        .get_mut(id)
                        .expect("In tests this account should be stored");
                    account.nonce = max(account.nonce, *new_nonce);
                    account.pub_key_hash = *new_pub_key_hash;
                }
                AccountUpdate::MintNFT { ref token, .. } => {
                    inner.tokens.insert(
                        token.id,
                        Token {
                            id: token.id,
                            address: token.address,
                            symbol: token.symbol.clone(),
                            decimals: 0,
                            kind: TokenKind::NFT,
                            is_nft: true,
                        },
                    );
                }
                AccountUpdate::RemoveNFT { ref token, .. } => {
                    inner.tokens.remove(&token.id);
                }
            }
        }
    }

    pub async fn start_transaction(&self) -> Self {
        self.clone()
    }

    pub async fn commit(self) {
        // Transactions are not supported, simply discard this reference.
    }

    pub async fn save_rollup_ops(&mut self, blocks: &[RollupOpsBlock]) {
        let mut inner = self.inner.borrow_mut();
        inner.rollups = blocks.to_vec();
        inner.storage_state = StorageUpdateState::Operations
    }

    pub async fn update_tree_state(&mut self, block: Block, accounts_updated: AccountUpdates) {
        let mut inner = self.inner.borrow_mut();

        let commit_op = Operation {
            action: Action::Commit,
            block: block.clone(),
            id: None,
        };

        let verify_op = Operation {
            action: Action::Verify {
                proof: Box::default(),
            },
            block: block.clone(),
            id: None,
        };

        inner.last_committed_block = commit_op.block.block_number;
        inner.last_verified_block = verify_op.block.block_number;
        drop(inner);

        self.commit_state_update(*block.block_number, accounts_updated);
        let mut inner = self.inner.borrow_mut();
        inner.storage_state = StorageUpdateState::None
        // TODO save operations
    }

    pub async fn apply_priority_op_data(
        &mut self,
        _priority_op_data: impl Iterator<Item = &PriorityOp>,
    ) -> Vec<SerialId> {
        Vec::new()
    }

    pub async fn store_token(&mut self, token: TokenInfo, token_id: TokenId) {
        let mut inner = self.inner.borrow_mut();
        let token = Token::new(
            token_id,
            token.address,
            &token.symbol,
            token.decimals,
            TokenKind::ERC20,
        );
        inner.tokens.insert(token_id, token);
    }

    pub async fn save_events_state(
        &mut self,
        block_events: &[BlockEvent],
        tokens: &[NewTokenEvent],
        _priority_op_data: &[PriorityOp],
        last_watched_eth_block_number: u64,
    ) {
        let mut inner = self.inner.borrow_mut();
        inner.events_state = block_events.to_vec();

        for &NewTokenEvent {
            id,
            address,
            eth_block_number: _,
        } in tokens
        {
            inner.tokens.insert(
                id,
                Token {
                    id,
                    address,
                    symbol: format!("ERC20-{}", *id),
                    decimals: 18,
                    kind: TokenKind::ERC20,
                    is_nft: false,
                },
            );
        }

        inner.last_watched_block = last_watched_eth_block_number;
        inner.storage_state = StorageUpdateState::Events;
    }

    pub async fn save_genesis_tree_state(
        &mut self,
        genesis_updates: &[(AccountId, AccountUpdate)],
    ) {
        self.commit_state_update(0, genesis_updates.to_vec());
    }

    pub async fn save_special_token(&mut self, token: Token) {
        let mut inner = self.inner.borrow_mut();
        inner.tokens.insert(token.id, token);
    }

    pub async fn get_block_events_state_from_storage(&mut self) -> EventsState {
        let inner = self.inner.borrow();
        let committed_events = self.load_committed_events_state();

        let verified_events = self.load_verified_events_state();

        EventsState {
            committed_events,
            verified_events,
            last_watched_eth_block_number: inner.last_watched_block,
            priority_op_data: Default::default(),
        }
    }

    pub async fn get_tree_state(&mut self) -> StoredTreeState {
        let inner = self.inner.borrow();
        // TODO find a way how to get unprocessed_prior_ops and fee_acc_id
        StoredTreeState {
            last_block_number: inner.last_verified_block,
            account_map: inner.accounts.clone(),
            unprocessed_prior_ops: 0,
            fee_acc_id: AccountId(0),
        }
    }

    pub async fn get_ops_blocks_from_storage(&mut self) -> Vec<RollupOpsBlock> {
        self.inner.borrow().rollups.clone()
    }

    pub async fn update_eth_state(&mut self) {
        // Do nothing it needs only for database
    }

    pub async fn get_storage_state(&mut self) -> StorageUpdateState {
        self.inner.borrow().storage_state
    }

    pub async fn get_cached_tree_state(&mut self) -> Option<CachedTreeState> {
        None
    }

    pub async fn update_tree_cache(&mut self, _block_number: BlockNumber, _tree_cache: String) {
        // Inmemory storage doesn't support caching.
    }

    pub async fn get_max_priority_op_serial_id(&mut self) -> SerialId {
        let number_of_priority_ops = self
            .inner
            .borrow()
            .rollups
            .iter()
            .flat_map(|rollup| &rollup.ops)
            .filter(|op| op.is_priority_op())
            .count();
        number_of_priority_ops as SerialId
    }
}
