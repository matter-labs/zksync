use zksync_crypto::{merkle_tree::parallel_smt::SparseMerkleTreeSerializableCacheBN256, Fr};
// External uses
// Workspace uses
use zksync_types::{AccountMap, AccountUpdates, BlockNumber};
// Local uses
use self::postgres::PostgresImpl;

#[cfg(test)]
use self::mock::MockImpl;

#[cfg(test)]
pub(super) mod mock;
pub(super) mod postgres;

#[derive(Debug)]
pub(super) enum StateRestoreDb<'a, 'b> {
    Postgres(PostgresImpl<'a, 'b>),
    #[cfg(test)]
    Mock(MockImpl),
}

macro_rules! delegate_call {
    ($self:ident.$method:ident($($args:ident),*)) => {
        match $self {
            Self::Postgres(d) => d.$method($($args),*).await,
            #[cfg(test)]
            Self::Mock(d) => d.$method($($args),*).await,
        }
    }
}

impl<'a, 'b> From<&'a mut zksync_storage::StorageProcessor<'b>> for StateRestoreDb<'a, 'b> {
    fn from(storage: &'a mut zksync_storage::StorageProcessor<'b>) -> Self {
        Self::Postgres(PostgresImpl::new(storage))
    }
}

#[cfg(test)]
impl<'a, 'b> From<MockImpl> for StateRestoreDb<'a, 'b> {
    fn from(storage: MockImpl) -> Self {
        Self::Mock(storage)
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
