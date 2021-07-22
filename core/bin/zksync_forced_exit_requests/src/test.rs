use std::{ops::Sub, sync::Mutex};

use chrono::Utc;
use zksync_storage::chain::operations_ext::records::TxReceiptResponse;
use zksync_types::Nonce;
use zksync_types::{
    forced_exit_requests::{ForcedExitRequest, ForcedExitRequestId},
    tx::TxHash,
    AccountId, SignedZkSyncTx,
};

use super::core_interaction_wrapper::CoreInteractionWrapper;

pub struct MockCoreInteractionWrapper {
    pub nonce: Nonce,
    pub requests: Mutex<Vec<ForcedExitRequest>>,
    pub tx_receipt: Option<TxReceiptResponse>,
    pub sent_txs: Mutex<Vec<SignedZkSyncTx>>,
    // It is easier when keeping track of the deleted txs
    pub deleted_requests: Mutex<Vec<ForcedExitRequest>>,
}

impl Default for MockCoreInteractionWrapper {
    fn default() -> Self {
        Self {
            nonce: Nonce(0),
            requests: Mutex::new(vec![]),
            tx_receipt: Some(TxReceiptResponse {
                // All the values here don't matter except for success = true
                tx_hash: String::from("1212"),
                block_number: 120,
                success: true,
                verified: false,
                fail_reason: None,
                prover_run: None,
            }),
            sent_txs: Mutex::new(vec![]),
            deleted_requests: Mutex::new(vec![]),
        }
    }
}

impl MockCoreInteractionWrapper {
    fn lock_requests(&self) -> std::sync::MutexGuard<'_, Vec<ForcedExitRequest>> {
        self.requests.lock().expect("Failed to get the write lock")
    }

    fn get_request_index_by_id(&self, id: ForcedExitRequestId) -> anyhow::Result<usize> {
        let lock = self.lock_requests();

        let index_and_request = (*lock).iter().enumerate().find(|(_, item)| item.id == id);

        let index_option = index_and_request.map(|(index, _)| index);

        index_option.ok_or_else(|| anyhow::Error::msg("Element not found"))
    }

    fn lock_sent_txs(&self) -> std::sync::MutexGuard<'_, Vec<SignedZkSyncTx>> {
        self.sent_txs.lock().expect("Failed to get the write lock")
    }

    fn lock_deleted_requests(&self) -> std::sync::MutexGuard<'_, Vec<ForcedExitRequest>> {
        self.deleted_requests
            .lock()
            .expect("Failed to allocate deleted requests")
    }
}

#[async_trait::async_trait]
impl CoreInteractionWrapper for MockCoreInteractionWrapper {
    async fn get_nonce(&self, _account_id: AccountId) -> anyhow::Result<Option<Nonce>> {
        Ok(Some(self.nonce))
    }
    async fn get_unconfirmed_requests(&self) -> anyhow::Result<Vec<ForcedExitRequest>> {
        let requests = self.lock_requests();

        let unconfirmed_requests = requests
            .iter()
            .filter(|r| r.fulfilled_at.is_none())
            .cloned()
            .collect();

        Ok(unconfirmed_requests)
    }
    async fn set_fulfilled_at(&self, id: i64) -> anyhow::Result<()> {
        let index = self.get_request_index_by_id(id)?;
        let mut requests = self.lock_requests();

        requests[index].fulfilled_at = Some(Utc::now());

        Ok(())
    }
    async fn set_fulfilled_by(
        &self,
        id: ForcedExitRequestId,
        value: Option<Vec<TxHash>>,
    ) -> anyhow::Result<()> {
        let index = self.get_request_index_by_id(id)?;
        let mut requests = self.lock_requests();

        requests[index].fulfilled_by = value;

        Ok(())
    }
    async fn get_request_by_id(&self, id: i64) -> anyhow::Result<Option<ForcedExitRequest>> {
        let index = self.get_request_index_by_id(id);

        match index {
            Ok(i) => {
                let requests = self.lock_requests();
                Ok(Some(requests[i].clone()))
            }
            Err(_) => Ok(None),
        }
    }

    async fn get_receipt(&self, _tx_hash: TxHash) -> anyhow::Result<Option<TxReceiptResponse>> {
        Ok(self.tx_receipt.clone())
    }

    async fn send_and_save_txs_batch(
        &self,
        request: &ForcedExitRequest,
        mut txs: Vec<SignedZkSyncTx>,
    ) -> anyhow::Result<Vec<TxHash>> {
        let hashes: Vec<TxHash> = txs.iter().map(|tx| tx.hash()).collect();

        self.lock_sent_txs().append(&mut txs);

        self.set_fulfilled_by(request.id, Some(hashes.clone()))
            .await?;

        Ok(hashes)
    }

    async fn get_oldest_unfulfilled_request(&self) -> anyhow::Result<Option<ForcedExitRequest>> {
        let requests = self.lock_requests();
        let unfulfilled_requests = requests.iter().filter(|r| r.fulfilled_by.is_none());
        let oldest = unfulfilled_requests.min_by_key(|req| req.created_at);

        Ok(oldest.cloned())
    }

    async fn delete_old_unfulfilled_requests(
        &self,
        deleting_threshold: chrono::Duration,
    ) -> anyhow::Result<()> {
        let mut requests = self.lock_requests();
        let mut deleted_requests = self.lock_deleted_requests();

        let oldest_allowed = Utc::now().sub(deleting_threshold);
        let (mut to_delete, mut to_remain): (Vec<_>, Vec<_>) = requests
            .iter()
            .cloned()
            .partition(|req| req.valid_until < oldest_allowed);

        requests.clear();
        requests.append(&mut to_remain);

        deleted_requests.append(&mut to_delete);
        Ok(())
    }

    async fn check_forced_exit_request(
        &self,
        _request: &ForcedExitRequest,
    ) -> anyhow::Result<bool> {
        // For tests it is better to just return true all the time
        Ok(true)
    }
}

pub fn add_request(requests: &Mutex<Vec<ForcedExitRequest>>, new_request: ForcedExitRequest) {
    let mut lock = requests.lock().unwrap();

    lock.push(new_request);
}
