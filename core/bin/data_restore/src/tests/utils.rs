use std::future::Future;

use crate::data_restore_driver::StorageUpdateState;
use crate::events::{BlockEvent, EventType};
use crate::events_state::{EventsState, NewTokenEvent};
use crate::rollup_ops::RollupOpsBlock;
use crate::storage_interactor::StorageInteractor;
use std::cmp::max;
use std::collections::HashMap;
use web3::types::Address;
use web3::{
    types::H256,
    types::{Bytes, Log},
    RequestId, Transport,
};
use zksync_types::block::Block;
use zksync_types::{
    Account, AccountId, AccountMap, AccountUpdate, AccountUpdates, Action, EncodedProofPlonk,
    Operation, Token, TokenGenesisListItem,
};

#[derive(Debug, Clone)]
pub(crate) struct FakeTransport;

impl Transport for FakeTransport {
    type Out = Box<dyn Future<Output = Result<jsonrpc_core::Value, web3::Error>> + Send + Unpin>;

    fn prepare(
        &self,
        _method: &str,
        _params: Vec<jsonrpc_core::Value>,
    ) -> (RequestId, jsonrpc_core::Call) {
        unreachable!()
    }

    fn send(&self, _id: RequestId, _request: jsonrpc_core::Call) -> Self::Out {
        unreachable!()
    }
}

pub(crate) fn u32_to_32bytes(value: u32) -> [u8; 32] {
    let mut bytes = [0u8; 32];
    let bytes_value = value.to_be_bytes();
    // Change only the last 4 bytes, which are represent u32
    bytes[28..32].clone_from_slice(&bytes_value);
    bytes
}

pub(crate) fn create_log(
    topic: H256,
    additional_topics: Vec<H256>,
    data: Bytes,
    block_number: u32,
    transaction_hash: H256,
) -> Log {
    let mut topics = vec![topic];
    topics.extend(additional_topics);
    Log {
        address: [1u8; 20].into(),
        topics,
        data,
        block_hash: None,
        block_number: Some(block_number.into()),
        transaction_hash: Some(transaction_hash),
        transaction_index: Some(0.into()),
        log_index: Some(0.into()),
        transaction_log_index: Some(0.into()),
        log_type: Some("mined".into()),
        removed: None,
    }
}

pub struct InMemoryStorageInteractor {
    rollups: Vec<RollupOpsBlock>,
    storage_state: StorageUpdateState,
    tokens: HashMap<u16, Token>,
    events_state: Vec<BlockEvent>,
    last_watched_block: u64,
    last_committed_block: u64,
    last_verified_block: u64,
    accounts: AccountMap,
}

#[async_trait::async_trait]
impl StorageInteractor for InMemoryStorageInteractor {
    async fn save_rollup_ops(&mut self, blocks: &[RollupOpsBlock]) {
        self.rollups = blocks.to_vec();
        self.storage_state = StorageUpdateState::Operations
    }

    async fn update_tree_state(&mut self, block: Block, accounts_updated: AccountUpdates) {
        let commit_op = Operation {
            action: Action::Commit,
            block: block.clone(),
            id: None,
        };

        let verify_op = Operation {
            action: Action::Verify {
                proof: Box::new(EncodedProofPlonk::default()),
            },
            block: block.clone(),
            id: None,
        };

        self.last_committed_block = commit_op.block.block_number as u64;
        self.last_verified_block = verify_op.block.block_number as u64;

        self.commit_state_update(block.block_number, accounts_updated);
        self.storage_state = StorageUpdateState::None
        // TODO save operations
    }

    async fn store_token(&mut self, token: TokenGenesisListItem, token_id: u16) {
        let token = Token {
            id: token_id,
            symbol: token.symbol,
            address: token.address[2..]
                .parse()
                .expect("failed to parse token address"),
            decimals: token.decimals,
        };
        self.tokens.insert(token_id, token);
    }

    async fn save_events_state(
        &mut self,
        block_events: &[BlockEvent],
        tokens: &[NewTokenEvent],
        last_watched_eth_block_number: u64,
    ) {
        self.events_state = block_events.to_vec();

        for &NewTokenEvent { id, address } in tokens {
            self.tokens.insert(
                id,
                Token {
                    id,
                    address,
                    symbol: format!("ERC20-{}", id),
                    decimals: 18,
                },
            );
        }

        self.last_watched_block = last_watched_eth_block_number;
        self.storage_state = StorageUpdateState::Events;
    }

    async fn save_genesis_tree_state(&mut self, genesis_acc_update: AccountUpdate) {
        self.commit_state_update(0, vec![(0, genesis_acc_update)]);
    }

    async fn get_block_events_state_from_storage(&mut self) -> EventsState {
        let committed_events = self.load_committed_events_state();

        let verified_events = self.load_verified_events_state();

        EventsState {
            committed_events,
            verified_events,
            last_watched_eth_block_number: self.last_watched_block,
        }
    }

    async fn get_tree_state(&mut self) -> (u32, AccountMap, u64, u32) {
        // TODO find a way how to get unprocessed_prior_ops and fee_acc_id
        (self.last_verified_block as u32, self.accounts.clone(), 0, 0)
    }

    async fn get_ops_blocks_from_storage(&mut self) -> Vec<RollupOpsBlock> {
        self.rollups.clone()
    }

    async fn update_eth_state(&mut self) {
        // Do nothing it needs only for database
    }

    async fn get_storage_state(&mut self) -> StorageUpdateState {
        self.storage_state
    }
}

impl InMemoryStorageInteractor {
    pub fn new() -> Self {
        Self {
            rollups: vec![],
            storage_state: StorageUpdateState::None,
            tokens: Default::default(),
            events_state: vec![],
            last_watched_block: 0,
            last_committed_block: 0,
            last_verified_block: 0,
            accounts: Default::default(),
        }
    }
    pub fn get_account_by_address(&self, address: &Address) -> Option<(AccountId, Account)> {
        let accounts: Vec<(AccountId, Account)> = self
            .accounts
            .iter()
            .filter(|(_, acc)| acc.address == *address)
            .map(|(acc_id, acc)| (*acc_id, acc.clone()))
            .collect();
        accounts.first().cloned()
    }
    fn load_verified_events_state(&self) -> Vec<BlockEvent> {
        self.events_state
            .clone()
            .into_iter()
            .filter(|event| event.block_type == EventType::Verified)
            .collect()
    }
    pub(crate) fn load_committed_events_state(&self) -> Vec<BlockEvent> {
        // TODO avoid clone
        self.events_state
            .clone()
            .into_iter()
            .filter(|event| event.block_type == EventType::Committed)
            .collect()
    }
    fn commit_state_update(
        &mut self,
        first_update_order_id: u32,
        accounts_updated: AccountUpdates,
    ) {
        let update_order_ids =
            first_update_order_id..first_update_order_id + accounts_updated.len() as u32;

        for (_, (id, upd)) in update_order_ids.zip(accounts_updated.iter()) {
            match *upd {
                AccountUpdate::Create { ref address, nonce } => {
                    let (mut acc, _) = Account::create_account(*id, *address);
                    acc.nonce = nonce;
                    self.accounts.insert(*id, acc);
                }
                AccountUpdate::Delete {
                    ref address,
                    nonce: _,
                } => {
                    let (acc_id, _) = self.get_account_by_address(address).unwrap();
                    self.accounts.remove(&acc_id);
                }
                AccountUpdate::UpdateBalance {
                    balance_update: (token, _, ref new_balance),
                    old_nonce: _,
                    new_nonce,
                } => {
                    let account = self
                        .accounts
                        .get_mut(id)
                        .expect("In tests this account should be stored");
                    account.set_balance(token, new_balance.clone());
                    account.nonce = max(account.nonce, new_nonce);
                }
                AccountUpdate::ChangePubKeyHash {
                    old_pub_key_hash: _,
                    ref new_pub_key_hash,
                    old_nonce: _,
                    new_nonce,
                } => {
                    let account = self
                        .accounts
                        .get_mut(id)
                        .expect("In tests this account should be stored");
                    account.nonce = max(account.nonce, new_nonce);
                    account.pub_key_hash = new_pub_key_hash.clone();
                }
            }
        }
    }
}
