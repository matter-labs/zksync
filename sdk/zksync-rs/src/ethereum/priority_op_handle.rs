//! This module contains the handler for sent priority operations.

use std::time::{Duration, Instant};

use zksync_types::PriorityOpId;

use crate::{
    error::ClientError,
    provider::Provider,
    types::{BlockInfo, EthOpInfo},
};

/// Handle for priority operations, providing an interface to control its execution.
/// For obtained handle it's possible to set the polling interval, commit timeout
/// and verify timeout values.
///
/// By default, awaiting for transaction may run up to forever, and the polling is
/// performed once a second.
#[derive(Debug)]
pub struct PriorityOpHandle<P: Provider> {
    serial_id: PriorityOpId,
    provider: P,
    polling_interval: Duration,
    commit_timeout: Option<Duration>,
    verify_timeout: Option<Duration>,
}

impl<P: Provider> PriorityOpHandle<P> {
    pub fn new(serial_id: PriorityOpId, provider: P) -> Self {
        Self {
            serial_id,
            provider,
            polling_interval: Duration::from_secs(1), // 1 second.
            commit_timeout: None,                     // Wait until forever
            verify_timeout: None,                     // Wait until forever
        }
    }

    const MIN_POLLING_INTERVAL: Duration = Duration::from_millis(200);

    /// Sets the polling interval. Must be at least 200 milliseconds.
    pub fn polling_interval(&mut self, polling_interval: Duration) -> Result<(), ClientError> {
        if polling_interval >= Self::MIN_POLLING_INTERVAL {
            self.polling_interval = polling_interval;
            Ok(())
        } else {
            Err(ClientError::PollingIntervalIsTooSmall)
        }
    }

    /// Returns the priority operation serial ID.
    pub fn serial_id(&self) -> PriorityOpId {
        self.serial_id
    }

    /// Sets the timeout for commit operation.
    /// With this value set, `PriorityOpHandle::wait_for_commit` will return a `ClientError::OperationTimeout`
    /// error if block will not be committed within provided time range.
    pub fn commit_timeout(mut self, commit_timeout: Duration) -> Self {
        self.commit_timeout = Some(commit_timeout);
        self
    }

    /// Sets the timeout for commit operation.
    /// With this value set, `PriorityOpHandle::wait_for_verify` will return a `ClientError::OperationTimeout`
    /// error if block will not be verified within provided time range.
    pub fn verify_timeout(mut self, verify_timeout: Duration) -> Self {
        self.verify_timeout = Some(verify_timeout);
        self
    }

    /// Awaits for the transaction commit and returns the information about its execution.
    pub async fn wait_for_commit(&self) -> Result<EthOpInfo, ClientError> {
        self.wait_for(|block| block.committed, self.commit_timeout)
            .await
    }

    /// Awaits for the transaction verification and returns the information about its execution.
    pub async fn wait_for_verify(&self) -> Result<EthOpInfo, ClientError> {
        self.wait_for(|block| block.verified, self.verify_timeout)
            .await
    }

    /// Awaits for the transaction to reach given state and returns the information about its execution.
    async fn wait_for<WaitPredicate>(
        &self,
        mut pred: WaitPredicate,
        timeout: Option<Duration>,
    ) -> Result<EthOpInfo, ClientError>
    where
        WaitPredicate: FnMut(&BlockInfo) -> bool,
    {
        let mut timer = tokio::time::interval(self.polling_interval);
        let start = Instant::now();

        loop {
            timer.tick().await;

            if let Some(timeout) = timeout {
                if start.elapsed() >= timeout {
                    return Err(ClientError::OperationTimeout);
                }
            }

            let response = self.provider.ethop_info(*self.serial_id as u32).await?;
            if let Some(block) = &response.block {
                if pred(block) {
                    return Ok(response);
                }
            }
        }
    }
}
