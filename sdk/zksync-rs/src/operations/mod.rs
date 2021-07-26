//! This file contains representation of not signed transactions and builders for them.

use std::time::{Duration, Instant};

use zksync_types::tx::TxHash;

use crate::{
    error::ClientError,
    provider::Provider,
    types::{BlockInfo, TransactionInfo},
};

pub use self::{
    change_pubkey::ChangePubKeyBuilder, mint_nft::MintNFTBuilder, transfer::TransferBuilder,
    transfer_nft::TransferNFTBuilder, withdraw::WithdrawBuilder, withdraw_nft::WithdrawNFTBuilder,
};

mod change_pubkey;
mod mint_nft;
mod transfer;
mod transfer_nft;
mod withdraw;
mod withdraw_nft;

/// Handle for transaction, providing an interface to control its execution.
/// For obtained handle it's possible to set the polling interval, commit timeout
/// and verify timeout values.
///
/// By default, awaiting for transaction may run up to forever, and the polling is
/// performed once a second.
#[derive(Debug)]
pub struct SyncTransactionHandle<P: Provider> {
    hash: TxHash,
    provider: P,
    polling_interval: Duration,
    commit_timeout: Option<Duration>,
    verify_timeout: Option<Duration>,
}

impl<P: Provider> SyncTransactionHandle<P> {
    pub fn new(hash: TxHash, provider: P) -> Self {
        Self {
            hash,
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

    /// Awaits for the transaction commit and returns the information about its execution.
    pub async fn wait_for_commit(&self) -> Result<TransactionInfo, ClientError> {
        self.wait_for(|block| block.committed, self.commit_timeout)
            .await
    }

    /// Awaits for the transaction verification and returns the information about its execution.
    pub async fn wait_for_verify(&self) -> Result<TransactionInfo, ClientError> {
        self.wait_for(|block| block.verified, self.verify_timeout)
            .await
    }

    /// Awaits for the transaction to reach given state and returns the information about its execution.
    async fn wait_for<WaitPredicate>(
        &self,
        condition: WaitPredicate,
        timeout: Option<Duration>,
    ) -> Result<TransactionInfo, ClientError>
    where
        WaitPredicate: Fn(&BlockInfo) -> bool,
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

            let response = self.provider.tx_info(self.hash).await?;
            if let Some(block) = &response.block {
                if condition(block) {
                    return Ok(response);
                }
            }
        }
    }
}
