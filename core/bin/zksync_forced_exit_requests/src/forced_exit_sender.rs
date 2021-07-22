use std::ops::AddAssign;

use chrono::{DateTime, Utc};
use num::BigUint;
use tokio::time;

use zksync_config::ZkSyncConfig;

use zksync_types::{
    forced_exit_requests::ForcedExitRequest, tx::TimeRange, tx::TxHash, AccountId, Address, Nonce,
    TokenId, ZkSyncTx,
};

use zksync_types::ForcedExit;
use zksync_types::SignedZkSyncTx;

use crate::{core_interaction_wrapper::CoreInteractionWrapper, utils};

use super::utils::{Engine, PrivateKey};
use crate::utils::read_signing_key;

// We try to process a request 3 times before sending warnings in the console
const PROCESSING_ATTEMPTS: u32 = 3;

#[async_trait::async_trait]
pub trait ForcedExitSender {
    async fn process_request(&self, amount: BigUint, submission_time: DateTime<Utc>);
}

pub struct MempoolForcedExitSender<T: CoreInteractionWrapper> {
    core_interaction_wrapper: T,
    config: ZkSyncConfig,
    forced_exit_sender_account_id: AccountId,
    sender_private_key: PrivateKey<Engine>,
}

#[async_trait::async_trait]
impl<T: CoreInteractionWrapper + Sync + Send> ForcedExitSender for MempoolForcedExitSender<T> {
    async fn process_request(&self, amount: BigUint, submission_time: DateTime<Utc>) {
        let mut attempts: u32 = 0;
        // Typically this should not run any longer than 1 iteration
        // In case something bad happens we do not want the server crush because
        // of the forced_exit_requests component
        loop {
            let processing_attempt = self
                .try_process_request(amount.clone(), submission_time)
                .await;

            if processing_attempt.is_ok() {
                return;
            } else {
                attempts += 1;
            }

            if attempts >= PROCESSING_ATTEMPTS {
                // We should not get stuck processing requests that possibly could never be processed
                break;
            }
        }
    }
}

impl<T: CoreInteractionWrapper> MempoolForcedExitSender<T> {
    pub fn new(
        core_interaction_wrapper: T,
        config: ZkSyncConfig,
        forced_exit_sender_account_id: AccountId,
    ) -> Self {
        let sender_private_key = hex::decode(&config.forced_exit_requests.sender_private_key[2..])
            .expect("Decoding private key failed");
        let sender_private_key =
            read_signing_key(&sender_private_key).expect("Reading private key failed");

        Self {
            core_interaction_wrapper,
            config,
            forced_exit_sender_account_id,
            sender_private_key,
        }
    }

    pub fn build_forced_exit(
        &self,
        nonce: Nonce,
        target: Address,
        token: TokenId,
    ) -> SignedZkSyncTx {
        let tx = ForcedExit::new_signed(
            self.forced_exit_sender_account_id,
            target,
            token,
            BigUint::from(0u32),
            nonce,
            TimeRange::default(),
            &self.sender_private_key,
        )
        .expect("Failed to create signed ForcedExit transaction");

        SignedZkSyncTx {
            tx: ZkSyncTx::ForcedExit(Box::new(tx)),
            eth_sign_data: None,
        }
    }

    pub async fn build_transactions(
        &self,
        // storage: &mut StorageProcessor<'_>,
        fe_request: ForcedExitRequest,
    ) -> anyhow::Result<Vec<SignedZkSyncTx>> {
        let mut sender_nonce = self
            .core_interaction_wrapper
            .get_nonce(self.forced_exit_sender_account_id)
            .await?
            .expect("Forced Exit sender account does not have nonce");

        let mut transactions: Vec<SignedZkSyncTx> = vec![];

        for token in fe_request.tokens.into_iter() {
            transactions.push(self.build_forced_exit(sender_nonce, fe_request.target, token));
            sender_nonce.add_assign(1);
        }

        Ok(transactions)
    }

    // Returns the id the request if it should be fulfilled,
    // error otherwise
    pub fn check_request(
        &self,
        amount: BigUint,
        submission_time: DateTime<Utc>,
        request: Option<ForcedExitRequest>,
    ) -> bool {
        let request = match request {
            Some(r) => r,
            None => {
                // The request does not exit, we should not process it
                return false;
            }
        };

        if request.fulfilled_at.is_some() {
            // We should not re-process requests that were fulfilled before
            return false;
        }

        request.valid_until > submission_time && request.price_in_wei == amount
    }

    // Awaits until the request is complete
    pub async fn await_unconfirmed_request(
        &self,
        request: &ForcedExitRequest,
    ) -> anyhow::Result<()> {
        let hashes = request.fulfilled_by.clone();

        if let Some(hashes) = hashes {
            for hash in hashes.into_iter() {
                self.wait_until_comitted(hash).await?;
                self.core_interaction_wrapper
                    .set_fulfilled_at(request.id)
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn await_unconfirmed(&mut self) -> anyhow::Result<()> {
        let unfullied_requests = self
            .core_interaction_wrapper
            .get_unconfirmed_requests()
            .await?;

        for request in unfullied_requests.into_iter() {
            let await_result = self.await_unconfirmed_request(&request).await;

            if await_result.is_err() {
                // A transaction has failed. That is not intended.
                // We can safely cancel such transaction, since we will re-try to
                // send it again later
                vlog::error!(
                    "A previously sent forced exit transaction has failed. Canceling the tx."
                );
                self.core_interaction_wrapper
                    .set_fulfilled_by(request.id, None)
                    .await?;
            }
        }

        Ok(())
    }

    pub async fn wait_until_comitted(&self, tx_hash: TxHash) -> anyhow::Result<()> {
        let timeout_millis: u64 = 120000;
        let poll_interval_millis: u64 = 200;
        let poll_interval = time::Duration::from_millis(poll_interval_millis);
        let mut timer = time::interval(poll_interval);

        let mut time_passed: u64 = 0;

        loop {
            if time_passed >= timeout_millis {
                // If a transaction takes more than 2 minutes to commit we consider the server
                // broken and panic
                panic!("Comitting ForcedExit transaction failed!");
            }

            let receipt = self.core_interaction_wrapper.get_receipt(tx_hash).await?;

            if let Some(tx_receipt) = receipt {
                if tx_receipt.success {
                    return Ok(());
                } else {
                    return Err(anyhow::Error::msg("ForcedExit transaction failed"));
                }
            }

            timer.tick().await;
            time_passed += poll_interval_millis;
        }
    }

    pub async fn try_process_request(
        &self,
        amount: BigUint,
        submission_time: DateTime<Utc>,
    ) -> anyhow::Result<()> {
        let (id, amount) = utils::extract_id_from_amount(
            amount,
            self.config.forced_exit_requests.digits_in_id as u32,
        );

        let fe_request = self.core_interaction_wrapper.get_request_by_id(id).await?;

        let fe_request = if self.check_request(amount, submission_time, fe_request.clone()) {
            // The self.check_request already checked that the fe_request is Some(_)
            fe_request.unwrap()
        } else {
            // The request was not valid, that's fine
            return Ok(());
        };

        let txs = self.build_transactions(fe_request.clone()).await?;

        // Right before sending the transactions we must check if the request is possible at all
        let is_request_possible = self
            .core_interaction_wrapper
            .check_forced_exit_request(&fe_request)
            .await?;
        if !is_request_possible {
            // If not possible at all, return without sending any transactions
            return Ok(());
        }
        let hashes = self
            .core_interaction_wrapper
            .send_and_save_txs_batch(&fe_request, txs)
            .await?;

        // We wait only for the first transaction to complete since the transactions
        // are sent in a batch
        self.wait_until_comitted(hashes[0]).await?;
        self.core_interaction_wrapper.set_fulfilled_at(id).await?;

        Ok(())
    }
}
#[cfg(test)]
mod test {
    use std::{
        ops::{Add, Mul},
        str::FromStr,
    };

    use zksync_config::ForcedExitRequestsConfig;

    use super::*;
    use crate::test::{add_request, MockCoreInteractionWrapper};

    // Just a random number for tests
    const TEST_ACCOUNT_FORCED_EXIT_SENDER_ID: u32 = 12;

    fn get_test_forced_exit_sender(
        config: Option<ZkSyncConfig>,
    ) -> MempoolForcedExitSender<MockCoreInteractionWrapper> {
        let core_interaction_wrapper = MockCoreInteractionWrapper::default();

        let config = config.unwrap_or_else(ZkSyncConfig::from_env);

        MempoolForcedExitSender::new(
            core_interaction_wrapper,
            config,
            AccountId(TEST_ACCOUNT_FORCED_EXIT_SENDER_ID),
        )
    }

    #[tokio::test]
    async fn test_forced_exit_sender() {
        let day = chrono::Duration::days(1);

        let config = ZkSyncConfig::from_env();
        let forced_exit_requests = ForcedExitRequestsConfig {
            // There must be 10 digits in id
            digits_in_id: 10,
            ..config.forced_exit_requests
        };
        let config = ZkSyncConfig {
            forced_exit_requests,
            ..config
        };

        let forced_exit_sender = get_test_forced_exit_sender(Some(config));

        add_request(
            &forced_exit_sender.core_interaction_wrapper.requests,
            ForcedExitRequest {
                id: 12,
                target: Address::random(),
                tokens: vec![TokenId(1)],
                price_in_wei: BigUint::from_str("10000000000").unwrap(),
                valid_until: Utc::now().add(day),
                created_at: Utc::now(),
                fulfilled_by: None,
                fulfilled_at: None,
            },
        );

        // Not the right amount, because not enough zeroes
        forced_exit_sender
            .process_request(BigUint::from_str("1000000012").unwrap(), Utc::now())
            .await;
        assert_eq!(
            forced_exit_sender
                .core_interaction_wrapper
                .sent_txs
                .lock()
                .unwrap()
                .len(),
            0
        );

        // Not the right amount, because id is not correct
        forced_exit_sender
            .process_request(BigUint::from_str("10000000001").unwrap(), Utc::now())
            .await;
        assert_eq!(
            forced_exit_sender
                .core_interaction_wrapper
                .sent_txs
                .lock()
                .unwrap()
                .len(),
            0
        );

        // The tranasction is correct, buuut it is expired
        forced_exit_sender
            .process_request(
                BigUint::from_str("10000000001").unwrap(),
                Utc::now().add(day.mul(3)),
            )
            .await;

        assert_eq!(
            forced_exit_sender
                .core_interaction_wrapper
                .sent_txs
                .lock()
                .unwrap()
                .len(),
            0
        );

        // The transaction is correct
        forced_exit_sender
            .process_request(BigUint::from_str("10000000012").unwrap(), Utc::now())
            .await;

        assert_eq!(
            forced_exit_sender
                .core_interaction_wrapper
                .sent_txs
                .lock()
                .unwrap()
                .len(),
            1
        );
    }
}
