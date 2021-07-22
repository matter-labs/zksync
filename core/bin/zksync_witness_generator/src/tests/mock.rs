// Built-in
use std::clone::Clone;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
// External uses
use chrono::Utc;
use tokio::sync::RwLock;
use tokio::time::delay_for;
// Workspace uses
use zksync_crypto::params::account_tree_depth;
use zksync_crypto::proof::{AggregatedProof, SingleProof};
use zksync_storage::chain::block::records::AccountTreeCache;
use zksync_storage::prover::records::{StorageBlockWitness, StorageProverJobQueue, StoredProof};
use zksync_storage::StorageProcessor;
use zksync_types::{
    aggregated_operations::{AggregatedActionType, AggregatedOperation},
    block::Block,
    prover::{ProverJob, ProverJobStatus, ProverJobType},
    AccountId, AccountMap, AccountTree, AccountUpdates, Address, BlockNumber,
};
// Local uses
use crate::DatabaseInterface;

#[derive(Clone)]
pub struct MockDatabase {
    /// Next free id and the prover job queue.
    prover_job_queue: Arc<RwLock<(i32, Vec<StorageProverJobQueue>)>>,
    proofs: Arc<RwLock<Vec<StoredProof>>>,
    block_witness: Arc<RwLock<Vec<StorageBlockWitness>>>,
    blocks: Arc<RwLock<Vec<Block>>>,
    account_tree_cache: Arc<RwLock<AccountTreeCache>>,
    accounts_state: Arc<RwLock<(u32, AccountMap)>>,
}

impl MockDatabase {
    pub fn new() -> Self {
        let (circuit_tree, accounts) = Self::get_default_tree_and_accounts();
        let tree_cache = serde_json::to_string(&circuit_tree.get_internals()).unwrap();

        Self {
            prover_job_queue: Arc::new(RwLock::new((0, Vec::new()))),
            proofs: Arc::new(RwLock::new(Vec::new())),
            block_witness: Arc::new(RwLock::new(Vec::new())),
            blocks: Arc::new(RwLock::new(Vec::new())),
            account_tree_cache: Arc::new(RwLock::new(AccountTreeCache {
                block: 0,
                tree_cache,
            })),
            accounts_state: Arc::new(RwLock::new((0, accounts))),
        }
    }

    pub fn get_default_tree_and_accounts() -> (AccountTree, AccountMap) {
        let mut tree = AccountTree::new(account_tree_depth());

        // Fee account
        let mut accounts = zksync_types::AccountMap::default();
        let validator_account = zksync_types::Account::default_with_address(
            &Address::from_str("34083bbd70d394110487feaa087da875a54624ec").unwrap(),
        );
        let validator_account_id = AccountId(0);
        accounts.insert(validator_account_id, validator_account.clone());

        tree.insert(0, validator_account);
        tree.root_hash();
        (tree, accounts)
    }

    pub async fn wait_for_stale_job_stale_idle() {
        delay_for(Duration::from_secs(10)).await;
    }

    pub async fn add_block(&self, block: Block) {
        self.blocks.write().await.push(block);
    }
}

#[async_trait::async_trait]
impl DatabaseInterface for MockDatabase {
    /// Creates a new database connection, used as a stub
    /// and nothing will be sent through this connection.
    async fn acquire_connection(&self) -> anyhow::Result<StorageProcessor<'_>> {
        StorageProcessor::establish_connection().await
    }

    async fn add_prover_job_to_job_queue(
        &self,
        _: &mut StorageProcessor<'_>,
        first_block: BlockNumber,
        last_block: BlockNumber,
        job_data: serde_json::Value,
        job_priority: i32,
        job_type: ProverJobType,
    ) -> anyhow::Result<()> {
        let mut prover_job_queue = self.prover_job_queue.write().await;
        let id = prover_job_queue.0;
        (*prover_job_queue).0 += 1;

        let new_job = StorageProverJobQueue {
            job_status: ProverJobStatus::Idle.to_number(),
            first_block: i64::from(*first_block),
            last_block: i64::from(*last_block),
            job_type: job_type.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            updated_by: "server_add_job".to_string(),
            id,
            job_priority,
            job_data,
        };

        prover_job_queue.1.push(new_job);

        Ok(())
    }

    async fn load_last_block_prover_job_queue(
        &self,
        _: &mut StorageProcessor<'_>,
        job_type: ProverJobType,
    ) -> anyhow::Result<BlockNumber> {
        let block_number = self
            .prover_job_queue
            .read()
            .await
            .1
            .iter()
            .filter(|job| job.job_type == job_type.to_string())
            .map(|job| job.last_block)
            .max()
            .unwrap_or_default();

        Ok(BlockNumber(block_number as u32))
    }

    async fn pending_jobs_count(&self, _: &mut StorageProcessor<'_>) -> anyhow::Result<u32> {
        let count = self
            .prover_job_queue
            .read()
            .await
            .1
            .iter()
            .filter(|job| job.job_status == ProverJobStatus::Idle.to_number())
            .count();

        Ok(count as u32)
    }

    async fn load_aggregated_op_that_affects_block(
        &self,
        _: &mut StorageProcessor<'_>,
        _aggregated_action: AggregatedActionType,
        _block_number: BlockNumber,
    ) -> anyhow::Result<Option<(i64, AggregatedOperation)>> {
        Ok(None)
    }

    async fn load_proof(
        &self,
        _: &mut StorageProcessor<'_>,
        block_number: BlockNumber,
    ) -> anyhow::Result<Option<SingleProof>> {
        let proofs = self.proofs.read().await;
        let single_proof = proofs
            .iter()
            .find(|proof| proof.block_number == *block_number as i64)
            .map(|stored| serde_json::from_value(stored.proof.clone()).unwrap());

        Ok(single_proof)
    }

    async fn mark_stale_jobs_as_idle(&self, _: &mut StorageProcessor<'_>) -> anyhow::Result<()> {
        let now = Utc::now();
        let prover_job_queue = &mut self.prover_job_queue.write().await.1;

        for job in prover_job_queue.iter_mut() {
            if now - job.updated_at > chrono::Duration::seconds(10) {
                job.job_status = ProverJobStatus::Idle.to_number();
                job.updated_at = now;
                job.updated_by = "server_clean_idle".to_string();
            }
        }

        Ok(())
    }

    async fn load_last_verified_block(
        &self,
        _: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<BlockNumber> {
        Ok(BlockNumber(0))
    }

    async fn load_block(
        &self,
        _: &mut StorageProcessor<'_>,
        block_number: BlockNumber,
    ) -> anyhow::Result<Option<Block>> {
        let storage_block = self.blocks.read().await;

        let block = storage_block
            .iter()
            .find(|block| block.block_number == block_number)
            .map(|block| (*block).clone());

        Ok(block)
    }

    async fn load_account_tree_cache(
        &self,
        _: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<Option<(BlockNumber, serde_json::Value)>> {
        let account_tree_cache = self.account_tree_cache.read().await;
        let result = (
            BlockNumber(account_tree_cache.block as u32),
            serde_json::from_str(&account_tree_cache.tree_cache)
                .expect("Failed to deserialize Account Tree Cache"),
        );

        Ok(Some(result))
    }

    async fn load_idle_prover_job_from_job_queue(
        &self,
        _: &mut StorageProcessor<'_>,
    ) -> anyhow::Result<Option<ProverJob>> {
        let prover_job_queue = &mut self.prover_job_queue.write().await.1;
        let idle_prover_job = prover_job_queue
            .iter_mut()
            .filter(|job| job.job_status == ProverJobStatus::Idle.to_number())
            .max_by_key(|job| (job.job_priority, job.id));

        let prover_job = if let Some(job) = idle_prover_job {
            job.job_status = ProverJobStatus::InProgress.to_number();
            job.updated_at = Utc::now();
            job.updated_by = "server_give_job".to_string();

            Some(ProverJob::new(
                job.id,
                BlockNumber(job.first_block as u32),
                BlockNumber(job.last_block as u32),
                job.job_data.clone(),
            ))
        } else {
            None
        };

        Ok(prover_job)
    }

    async fn record_prover_is_working(
        &self,
        _: &mut StorageProcessor<'_>,
        job_id: i32,
        prover_name: &str,
    ) -> anyhow::Result<()> {
        let prover_job_queue = &mut self.prover_job_queue.write().await.1;
        let prover_job = prover_job_queue.iter_mut().find(|job| job.id == job_id);

        if let Some(job) = prover_job {
            job.updated_at = Utc::now();
            job.updated_by = prover_name.to_string();
        }

        Ok(())
    }

    async fn store_proof(
        &self,
        _: &mut StorageProcessor<'_>,
        job_id: i32,
        block_number: BlockNumber,
        proof: &SingleProof,
    ) -> anyhow::Result<()> {
        let prover_job_queue = &mut self.prover_job_queue.write().await.1;
        let prover_job = prover_job_queue.iter_mut().find(|job| job.id == job_id);

        if let Some(job) = prover_job {
            job.updated_at = Utc::now();
            job.job_status = ProverJobStatus::Done.to_number();
            job.updated_by = "server_finish_job".to_string();
        }
        let proof = StoredProof {
            block_number: i64::from(*block_number),
            created_at: Utc::now(),
            proof: serde_json::to_value(proof).unwrap(),
        };
        self.proofs.write().await.push(proof);

        Ok(())
    }

    async fn store_aggregated_proof(
        &self,
        _: &mut StorageProcessor<'_>,
        _job_id: i32,
        _first_block: BlockNumber,
        _last_block: BlockNumber,
        _proof: &AggregatedProof,
    ) -> anyhow::Result<()> {
        unreachable!();
    }

    async fn record_prover_stop(
        &self,
        _: &mut StorageProcessor<'_>,
        prover_name: &str,
    ) -> anyhow::Result<()> {
        let prover_job_queue = &mut self.prover_job_queue.write().await.1;

        for job in prover_job_queue.iter_mut() {
            if job.updated_by == prover_name
                && job.job_status == ProverJobStatus::InProgress.to_number()
            {
                job.job_status = ProverJobStatus::Idle.to_number();
                job.updated_at = Utc::now();
            }
        }

        Ok(())
    }

    async fn load_committed_state(
        &self,
        _: &mut StorageProcessor<'_>,
        _block: Option<BlockNumber>,
    ) -> anyhow::Result<(BlockNumber, AccountMap)> {
        let (last_block, accounts) = self.accounts_state.read().await.clone();
        Ok((BlockNumber(last_block), accounts))
    }

    async fn load_state_diff(
        &self,
        _: &mut StorageProcessor<'_>,
        _from_block: BlockNumber,
        _to_block: Option<BlockNumber>,
    ) -> anyhow::Result<Option<(BlockNumber, AccountUpdates)>> {
        Ok(None)
    }

    async fn store_account_tree_cache(
        &self,
        _: &mut StorageProcessor<'_>,
        block: BlockNumber,
        tree_cache: serde_json::Value,
    ) -> anyhow::Result<()> {
        if *block == 0 {
            return Ok(());
        }
        let tree_cache =
            serde_json::to_string(&tree_cache).expect("Failed to serialize Account Tree Cache");

        let mut account_tree_cache = self.account_tree_cache.write().await;
        *account_tree_cache = AccountTreeCache {
            block: i64::from(*block),
            tree_cache,
        };

        Ok(())
    }

    async fn load_witness(
        &self,
        _: &mut StorageProcessor<'_>,
        block_number: BlockNumber,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        let block_witness = self.block_witness.read().await;
        let witness = block_witness
            .iter()
            .find(|witness| witness.block == *block_number as i64)
            .map(|w| serde_json::from_str(&w.witness).expect("Failed to deserialize witness"));

        Ok(witness)
    }

    async fn store_witness(
        &self,
        _: &mut StorageProcessor<'_>,
        block: BlockNumber,
        witness: serde_json::Value,
    ) -> anyhow::Result<()> {
        let witness_str = serde_json::to_string(&witness).expect("Failed to serialize witness");
        let mut block_witness = self.block_witness.write().await;
        let is_block_not_saved_yet = block_witness
            .iter()
            .find(|witness| witness.block == *block as i64)
            .is_none();

        if is_block_not_saved_yet {
            block_witness.push(StorageBlockWitness {
                block: *block as i64,
                witness: witness_str,
            });
        }

        Ok(())
    }
}
