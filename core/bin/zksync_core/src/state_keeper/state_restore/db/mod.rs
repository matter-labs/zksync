// TODO: To not be annoyed by warnings while in development. If you see this line in the PR, tell me that I'm stupid.
#![allow(dead_code)]

use zksync_crypto::{merkle_tree::parallel_smt::SparseMerkleTreeSerializableCacheBN256, Fr};
// External uses
// Workspace uses
use zksync_types::{AccountMap, AccountUpdates, BlockNumber};
// Local uses
use self::{mock::MockImpl, postgres::PostgresImpl};

mod mock;
mod postgres;

#[derive(Debug)]
pub(super) enum StateRestoreDb<'a, 'b> {
    Postgres(PostgresImpl<'a, 'b>),
    Mock(MockImpl),
}

macro_rules! delegate_call {
    ($self:ident.$method:ident($($args:ident),*)) => {
        match $self {
            Self::Postgres(d) => d.$method($($args),*).await,
            Self::Mock(d) => d.$method($($args),*).await,
        }
    }
}

impl<'a, 'b> StateRestoreDb<'a, 'b> {
    pub(super) async fn load_last_committed_block(&mut self) -> BlockNumber {
        delegate_call!(self.load_last_committed_block())
    }

    pub(super) async fn load_last_cached_block(&mut self) -> Option<BlockNumber> {
        delegate_call!(self.load_last_cached_block())
    }

    pub(super) async fn load_state_diff(
        &mut self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Option<AccountUpdates> {
        delegate_call!(self.load_state_diff(from_block, to_block))
    }

    pub(super) async fn load_committed_state(
        &mut self,
        block: BlockNumber,
    ) -> (BlockNumber, AccountMap) {
        delegate_call!(self.load_committed_state(block))
    }

    pub(super) async fn load_verified_state(&mut self) -> (BlockNumber, AccountMap) {
        delegate_call!(self.load_verified_state())
    }

    pub(super) async fn load_account_tree_cache(
        &mut self,
        block: BlockNumber,
    ) -> SparseMerkleTreeSerializableCacheBN256 {
        delegate_call!(self.load_account_tree_cache(block))
    }

    pub(super) async fn store_account_tree_cache(
        &mut self,
        block: BlockNumber,
        account_tree_cache: SparseMerkleTreeSerializableCacheBN256,
    ) {
        delegate_call!(self.store_account_tree_cache(block, account_tree_cache))
    }

    pub(super) async fn load_block_hash_from_db(&mut self, block: BlockNumber) -> Fr {
        delegate_call!(self.load_block_hash_from_db(block))
    }
}
