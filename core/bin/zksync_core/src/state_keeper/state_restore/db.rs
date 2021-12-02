// TODO: To not be annoyed by warnings while in development. If you see this line in the PR, tell me that I'm stupid.
#![allow(dead_code)]

use zksync_crypto::{merkle_tree::parallel_smt::SparseMerkleTreeSerializableCacheBN256, Fr};
// External uses
// Workspace uses
use zksync_types::{AccountMap, AccountUpdates, BlockNumber};

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

#[derive(Debug)]
pub(super) struct PostgresImpl<'a, 'b> {
    storage: &'a mut zksync_storage::StorageProcessor<'b>,
}

impl<'a, 'b> PostgresImpl<'a, 'b> {
    pub(super) fn new(storage: &'a mut zksync_storage::StorageProcessor<'b>) -> Self {
        Self { storage }
    }

    pub(super) async fn load_last_committed_block(&mut self) -> BlockNumber {
        self.storage
            .chain()
            .block_schema()
            .get_last_saved_block()
            .await
            .expect("Can't load the last saved block")
    }

    pub(super) async fn load_last_cached_block(&mut self) -> Option<BlockNumber> {
        self.storage
            .chain()
            .block_schema()
            .get_last_block_with_account_tree_cache()
            .await
            .expect("Can't load the last block with cache")
    }

    pub(super) async fn load_state_diff(
        &mut self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Option<AccountUpdates> {
        self.storage
            .chain()
            .state_schema()
            .load_state_diff(from_block, Some(to_block))
            .await
            .unwrap_or_else(|err| {
                panic!(
                    "Can't load the state diff for block range {}-{}: {}",
                    from_block, to_block, err
                )
            })
            .map(|(_block, updates)| updates)
    }

    pub(super) async fn load_committed_state(
        &mut self,
        block: BlockNumber,
    ) -> (BlockNumber, AccountMap) {
        self.storage
            .chain()
            .state_schema()
            .load_committed_state(Some(block))
            .await
            .expect("Can't load committed state")
    }

    pub(super) async fn load_verified_state(&mut self) -> (BlockNumber, AccountMap) {
        self.storage
            .chain()
            .state_schema()
            .load_verified_state()
            .await
            .expect("Can't load committed state")
    }

    pub(super) async fn load_account_tree_cache(
        &mut self,
        block: BlockNumber,
    ) -> SparseMerkleTreeSerializableCacheBN256 {
        let cache = self.storage
            .chain()
            .block_schema()
            .get_account_tree_cache_block(block)
            .await
            .expect("Can't load account tree cache")
            .unwrap_or_else(|| {
                panic!("Account tree cache was requested for block {}, for which it was checked to exist", block)
            });
        serde_json::from_value(cache).expect("Unable to decode tree cache")
    }

    pub(super) async fn store_account_tree_cache(
        &mut self,
        block: BlockNumber,
        account_tree_cache: SparseMerkleTreeSerializableCacheBN256,
    ) {
        let encoded_tree_cache =
            serde_json::to_value(account_tree_cache).expect("Unable to encode account tree cache");
        self.storage
            .chain()
            .block_schema()
            .store_account_tree_cache(block, encoded_tree_cache)
            .await
            .expect("Unable to store account tree cache in the database");
    }

    pub(super) async fn load_block_hash_from_db(&mut self, block: BlockNumber) -> Fr {
        self.storage
            .chain()
            .block_schema()
            .get_block(block)
            .await
            .unwrap_or_else(|err| panic!("Cannot load block {} from the database: {}", block, err))
            .unwrap_or_else(|| panic!("Block {} does not exist in the databse", block))
            .new_root_hash
    }
}

#[derive(Debug)]
pub(super) struct MockImpl {}

impl MockImpl {
    pub(super) fn new() -> Self {
        Self {}
    }

    pub(super) async fn load_last_committed_block(&mut self) -> BlockNumber {
        todo!()
    }

    pub(super) async fn load_last_cached_block(&mut self) -> Option<BlockNumber> {
        todo!()
    }

    pub(super) async fn load_state_diff(
        &mut self,
        from_block: BlockNumber,
        to_block: BlockNumber,
    ) -> Option<AccountUpdates> {
        todo!()
    }

    pub(super) async fn load_committed_state(
        &mut self,
        block: BlockNumber,
    ) -> (BlockNumber, AccountMap) {
        todo!()
    }

    pub(super) async fn load_verified_state(&mut self) -> (BlockNumber, AccountMap) {
        todo!()
    }

    pub(super) async fn load_account_tree_cache(
        &mut self,
        block: BlockNumber,
    ) -> SparseMerkleTreeSerializableCacheBN256 {
        todo!()
    }

    pub(super) async fn store_account_tree_cache(
        &mut self,
        block: BlockNumber,
        account_tree_cache: SparseMerkleTreeSerializableCacheBN256,
    ) {
        todo!()
    }

    pub(super) async fn load_block_hash_from_db(&mut self, block: BlockNumber) -> Fr {
        todo!()
    }
}
