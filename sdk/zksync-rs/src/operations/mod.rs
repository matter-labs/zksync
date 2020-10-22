//! This file contains representation of not signed transactions and builders for them.

use crate::{error::ClientError, provider::Provider, types::TransactionInfo};
use std::time::{Duration, Instant};
use zksync_types::tx::TxHash;

mod change_pubkey;
mod transfer;
mod withdraw;

pub use self::{
    change_pubkey::ChangePubKeyBuilder, transfer::TransferBuilder, withdraw::WithdrawBuilder,
};

/// Handle for transaction, providing an interface to control its execution.
/// For obtained handle it's possible to set the polling interval, commit timeout
/// and verify timeout values.
///
/// By default, awaiting for transaction may run up to forever, and the polling is
/// performed once a second.
#[derive(Debug)]
pub struct SyncTransactionHandle {
    hash: TxHash,
    provider: Provider,
    polling_interval: Duration,
    commit_timeout: Option<Duration>,
    verify_timeout: Option<Duration>,
}

impl SyncTransactionHandle {
    pub fn new(hash: TxHash, provider: Provider) -> Self {
        Self {
            hash,
            provider,
            polling_interval: Duration::from_secs(1), // 1 second.
            commit_timeout: None,                     // Wait until forever
            verify_timeout: None,                     // Wait until forever
        }
    }

    /// Sets the polling interval. Must be at least 200 milliseconds.
    pub fn polling_interval(&mut self, polling_interval: Duration) -> Result<(), ClientError> {
        if polling_interval >= Duration::from_millis(200) {
            self.polling_interval = polling_interval;
            Ok(())
        } else {
            Err(ClientError::PollingIntervalIsTooSmall)
        }
    }

    /// Returns the transaction hash.
    pub fn hash(&self) -> TxHash {
        self.hash
    }

    /// Sets the timeout for commit operation.
    /// With this value set, `SyncTransactionHandle::wait_for_commit` will return a `ClientError::OperationTimeout`
    /// error if block will not be committed within provided time range.
    pub fn commit_timeout(mut self, commit_timeout: Duration) -> Self {
        self.commit_timeout = Some(commit_timeout);
        self
    }

    /// Sets the timeout for commit operation.
    /// With this value set, `SyncTransactionHandle::wait_for_verify` will return a `ClientError::OperationTimeout`
    /// error if block will not be verified within provided time range.
    pub fn verify_timeout(mut self, verify_timeout: Duration) -> Self {
        self.verify_timeout = Some(verify_timeout);
        self
    }

    /// Awaits for the transaction commit and returns the information about execution.
    pub async fn wait_for_commit(&self) -> Result<TransactionInfo, ClientError> {
        let mut timer = tokio::time::interval(self.polling_interval);
        let start = Instant::now();

        loop {
            timer.tick().await;

            if let Some(commit_timeout) = self.commit_timeout {
                if start.elapsed() >= commit_timeout {
                    return Err(ClientError::OperationTimeout);
                }
            }

            let response = self.provider.tx_info(self.hash).await?;
            if let Some(block) = &response.block {
                if block.committed {
                    return Ok(response);
                }
            }
        }
    }

    /// Awaits for the transaction verification and returns the information about execution.
    pub async fn wait_for_verify(&self) -> Result<TransactionInfo, ClientError> {
        let mut timer = tokio::time::interval(self.polling_interval);
        let start = Instant::now();

        loop {
            timer.tick().await;

            if let Some(verify_timeout) = self.verify_timeout {
                if start.elapsed() >= verify_timeout {
                    return Err(ClientError::OperationTimeout);
                }
            }

            let response = self.provider.tx_info(self.hash).await?;
            if let Some(block) = &response.block {
                if block.verified {
                    return Ok(response);
                }
            }
        }
    }
}
