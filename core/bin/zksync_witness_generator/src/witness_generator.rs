use std::time::Instant;
// Built-in
use std::{thread, time};
// External
use futures::channel::mpsc;
use tokio::time::sleep;
// Workspace deps
use crate::database_interface::DatabaseInterface;
use zksync_circuit::serialization::ProverData;
use zksync_circuit::witness::utils::build_block_witness;
use zksync_crypto::circuit::CircuitAccountTree;
use zksync_crypto::params::account_tree_depth;
use zksync_types::block::Block;
use zksync_types::BlockNumber;
use zksync_utils::panic_notify::ThreadPanicNotify;

/// The essential part of this structure is `maintain` function
/// which runs forever and adds data to the database.
///
/// This will generate and store in db witnesses for blocks with indexes
/// start_block, start_block + block_step, start_block + 2*block_step, ...
pub struct WitnessGenerator<DB: DatabaseInterface> {
    /// Connection to the database.
    database: DB,
    /// Routine refresh interval.
    rounds_interval: time::Duration,

    start_block: BlockNumber,
    block_step: BlockNumber,
}

#[derive(Debug)]
enum BlockInfo {
    NotReadyBlock,
    WithWitness,
    NoWitness(Block),
}

impl<DB: DatabaseInterface> WitnessGenerator<DB> {
    /// Creates a new `WitnessGenerator` object.
    pub fn new(
        database: DB,
        rounds_interval: time::Duration,
        start_block: BlockNumber,
        block_step: BlockNumber,
    ) -> Self {
        Self {
            database,
            rounds_interval,
            start_block,
            block_step,
        }
    }

    /// Starts the thread running `maintain` method.
    pub fn start(self, panic_notify: mpsc::Sender<bool>) {
        thread::Builder::new()
            .name("prover_server_pool".to_string())
            .spawn(move || {
                let _panic_sentinel = ThreadPanicNotify(panic_notify);
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("Unable to build runtime for a witness generator");

                runtime.block_on(async move {
                    self.maintain().await;
                });
            })
            .expect("failed to start provers server");
    }

    /// Returns status of witness for block with index block_number
    async fn should_work_on_block(
        &self,
        block_number: BlockNumber,
    ) -> Result<BlockInfo, anyhow::Error> {
        let start = Instant::now();
        let mut storage = self.database.acquire_connection().await?;
        let mut transaction = storage.start_transaction().await?;
        let block = self
            .database
            .load_block(&mut transaction, block_number)
            .await?;
        let block_info = if let Some(block) = block {
            let witness = self
                .database
                .load_witness(&mut transaction, block_number)
                .await?;
            if witness.is_none() {
                BlockInfo::NoWitness(block)
            } else {
                BlockInfo::WithWitness
            }
        } else {
            BlockInfo::NotReadyBlock
        };
        transaction.commit().await?;
        metrics::histogram!("witness_generator", start.elapsed(), "stage" => "should_work_on_block");
        Ok(block_info)
    }

    async fn load_account_tree(
        &self,
        block: BlockNumber,
    ) -> Result<CircuitAccountTree, anyhow::Error> {
        let fn_start = Instant::now();

        let mut storage = self.database.acquire_connection().await?;

        let start = Instant::now();
        let mut circuit_account_tree = CircuitAccountTree::new(account_tree_depth());
        let cache = self.database.load_account_tree_cache(&mut storage).await?;
        metrics::histogram!("witness_generator", start.elapsed(), "stage" => "load_cache");

        let start = Instant::now();
        if let Some((cached_block, account_tree_cache)) = cache {
            let (_, accounts) = self
                .database
                .load_committed_state(&mut storage, Some(block))
                .await?;
            for (id, account) in accounts {
                circuit_account_tree.insert(*id, account.into());
            }
            circuit_account_tree.set_internals(serde_json::from_value(account_tree_cache)?);
            if block != cached_block {
                // There is no relevant cache, so we have to use some outdated cache and update the tree.
                if *block == *cached_block + 1 {
                    // Off by 1 misses are normally expected
                    metrics::increment_counter!("witness_generator.cache_access", "type" => "off_by_1");
                } else {
                    metrics::increment_counter!("witness_generator.cache_access", "type" => "miss");
                }

                vlog::info!("Reconstructing the cache for the block {} using the cached tree for the block {}", block, cached_block);

                let (_, accounts) = self
                    .database
                    .load_committed_state(&mut storage, Some(block))
                    .await?;
                if let Some((_, account_updates)) = self
                    .database
                    .load_state_diff(&mut storage, block, Some(cached_block))
                    .await?
                {
                    let mut updated_accounts = account_updates
                        .into_iter()
                        .map(|(id, _)| id)
                        .collect::<Vec<_>>();
                    updated_accounts.sort_unstable();
                    updated_accounts.dedup();
                    for idx in updated_accounts {
                        circuit_account_tree
                            .insert(*idx, accounts.get(&idx).cloned().unwrap_or_default().into());
                    }
                }
                circuit_account_tree.root_hash();
                metrics::histogram!("witness_generator", start.elapsed(), "stage" => "recreate_tree_from_cache");

                let start = Instant::now();
                let tree_cache = serde_json::to_string(&circuit_account_tree.get_internals())?;
                metrics::histogram!("tree_cache_size", tree_cache.len() as f64);

                self.database
                    .store_account_tree_cache(&mut storage, block, tree_cache)
                    .await?;
                metrics::histogram!("witness_generator", start.elapsed(), "stage" => "store_cache");
            } else {
                // There exists a cache for the block we are interested in.
                metrics::increment_counter!("witness_generator.cache_access", "type" => "hit");
            }
        } else {
            // There are no caches at all.
            let (_, accounts) = self
                .database
                .load_committed_state(&mut storage, Some(block))
                .await?;
            for (id, account) in accounts {
                circuit_account_tree.insert(*id, account.into());
            }
            circuit_account_tree.root_hash();

            metrics::histogram!("witness_generator", start.elapsed(), "stage" => "recreate_tree_from_scratch");

            let start = Instant::now();
            let tree_cache = serde_json::to_string(&circuit_account_tree.get_internals())?;
            metrics::histogram!("tree_cache_size", tree_cache.len() as f64);
            metrics::histogram!("witness_generator", start.elapsed(), "stage" => "serialize_cache");

            let start = Instant::now();
            self.database
                .store_account_tree_cache(&mut storage, block, tree_cache)
                .await?;
            metrics::histogram!("witness_generator", start.elapsed(), "stage" => "store_cache");
        }

        let start = Instant::now();
        if block != BlockNumber(0) {
            let storage_block = self
                .database
                .load_block(&mut storage, block)
                .await?
                .expect("Block for witness generator must exist");
            assert_eq!(
                storage_block.new_root_hash,
                circuit_account_tree.root_hash(),
                "account tree root hash restored incorrectly"
            );
        }
        metrics::histogram!("witness_generator", start.elapsed(), "stage" => "ensure_root_hash");

        metrics::histogram!("witness_generator", fn_start.elapsed(), "stage" => "load_account_tree");
        Ok(circuit_account_tree)
    }

    async fn prepare_witness_and_save_it(&self, block: Block) -> anyhow::Result<()> {
        let fn_start = Instant::now();
        let mut storage = self.database.acquire_connection().await?;

        let start = Instant::now();
        let mut circuit_account_tree = self.load_account_tree(block.block_number - 1).await?;
        metrics::histogram!("witness_generator", start.elapsed(), "stage" => "load_tree_full");

        let start = Instant::now();
        let witness: ProverData = build_block_witness(&mut circuit_account_tree, &block)?.into();
        metrics::histogram!("witness_generator", start.elapsed(), "stage" => "build_witness");

        let start = Instant::now();
        self.database
            .store_witness(
                &mut storage,
                block.block_number,
                serde_json::to_value(witness).expect("Witness serialize to json"),
            )
            .await?;
        metrics::histogram!("witness_generator", start.elapsed(), "stage" => "store_witness");

        metrics::histogram!("witness_generator", fn_start.elapsed(), "stage" => "prepare_witness_and_save_it");

        metrics::gauge!(
            "last_processed_block",
            block.block_number.0 as f64,
            "stage" => "witness_generator"
        );
        Ok(())
    }

    /// Returns next block for generating witness
    fn next_witness_block(
        current_block: BlockNumber,
        block_step: BlockNumber,
        block_info: &BlockInfo,
    ) -> BlockNumber {
        match block_info {
            BlockInfo::NotReadyBlock => current_block, // Keep waiting
            BlockInfo::WithWitness | BlockInfo::NoWitness(_) => {
                BlockNumber(*current_block + *block_step)
            } // Go to the next block
        }
    }

    /// Updates witness data in database in an infinite loop,
    /// awaiting `rounds_interval` time between updates.
    async fn maintain(self) {
        vlog::info!(
            "preparing prover data routine started with start_block({}), block_step({})",
            *self.start_block,
            *self.block_step
        );

        // Initialize counters for cache hits/misses.
        metrics::register_counter!("witness_generator.cache_access", "type" => "hit");
        metrics::register_counter!("witness_generator.cache_access", "type" => "off_by_1");
        metrics::register_counter!("witness_generator.cache_access", "type" => "miss");

        let mut current_block = self.start_block;
        loop {
            sleep(self.rounds_interval).await;
            let should_work = match self.should_work_on_block(current_block).await {
                Ok(should_work) => should_work,
                Err(err) => {
                    vlog::warn!("witness for block {} check failed: {}", current_block, err);
                    continue;
                }
            };

            let next_block = Self::next_witness_block(current_block, self.block_step, &should_work);
            if let BlockInfo::NoWitness(block) = should_work {
                let block_number = block.block_number;
                if let Err(err) = self.prepare_witness_and_save_it(block).await {
                    vlog::warn!("Witness generator ({},{}) failed to prepare witness for block: {}, err: {}",
                        self.start_block, self.block_step, block_number, err);
                    continue; // Retry the same block on the next iteration.
                }
            }

            // Update current block.
            current_block = next_block;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use zksync_crypto::Fr;
    use zksync_types::{AccountId, H256, U256};

    #[test]
    fn test_next_witness_block() {
        assert_eq!(
            WitnessGenerator::<Database>::next_witness_block(
                BlockNumber(3),
                BlockNumber(4),
                &BlockInfo::NotReadyBlock
            ),
            BlockNumber(3)
        );
        assert_eq!(
            WitnessGenerator::<Database>::next_witness_block(
                BlockNumber(3),
                BlockNumber(4),
                &BlockInfo::WithWitness
            ),
            BlockNumber(7)
        );
        let empty_block = Block::new(
            BlockNumber(0),
            Fr::default(),
            AccountId(0),
            vec![],
            (0, 0),
            0,
            U256::default(),
            U256::default(),
            H256::default(),
            0,
        );
        assert_eq!(
            WitnessGenerator::<Database>::next_witness_block(
                BlockNumber(3),
                BlockNumber(4),
                &BlockInfo::NoWitness(empty_block)
            ),
            BlockNumber(7)
        );
    }
}
