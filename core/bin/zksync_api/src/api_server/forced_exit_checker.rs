use crate::api_server::tx_sender::SubmitError;
use zksync_config::ZkSyncConfig;
use zksync_storage::StorageProcessor;
use zksync_types::Address;

use crate::internal_error;

use chrono::Utc;

#[async_trait::async_trait]
pub trait ForcedExitAccountAgeChecker {
    async fn check_forced_exit(
        &self,
        storage: &mut StorageProcessor<'_>,
        target_account_address: Address,
    ) -> Result<bool, SubmitError>;

    async fn validate_forced_exit(
        &self,
        storage: &mut StorageProcessor<'_>,
        target_account_address: Address,
    ) -> Result<(), SubmitError>;
}

#[derive(Clone)]
pub struct ForcedExitChecker {
    /// Mimimum age of the account for `ForcedExit` operations to be allowed.
    pub forced_exit_minimum_account_age: chrono::Duration,
}

impl ForcedExitChecker {
    pub fn new(config: &ZkSyncConfig) -> Self {
        let forced_exit_minimum_account_age = chrono::Duration::seconds(
            config.api.common.forced_exit_minimum_account_age_secs as i64,
        );

        Self {
            forced_exit_minimum_account_age,
        }
    }
}

#[async_trait::async_trait]
impl ForcedExitAccountAgeChecker for ForcedExitChecker {
    async fn check_forced_exit(
        &self,
        storage: &mut StorageProcessor<'_>,
        target_account_address: Address,
    ) -> Result<bool, SubmitError> {
        let account_age = storage
            .chain()
            .operations_ext_schema()
            .account_created_on(&target_account_address)
            .await
            .map_err(|err| internal_error!(err, target_account_address))?;

        match account_age {
            Some(age) if Utc::now() - age < self.forced_exit_minimum_account_age => Ok(false),
            None => Err(SubmitError::invalid_params("Target account does not exist")),

            Some(..) => Ok(true),
        }
    }

    async fn validate_forced_exit(
        &self,
        storage: &mut StorageProcessor<'_>,
        target_account_address: Address,
    ) -> Result<(), SubmitError> {
        let eligible = self
            .check_forced_exit(storage, target_account_address)
            .await?;

        if eligible {
            Ok(())
        } else {
            let msg = format!(
                "Target account exists less than required minimum amount ({} hours)",
                self.forced_exit_minimum_account_age.num_hours()
            );

            Err(SubmitError::InvalidParams(msg))
        }
    }
}

pub struct DummyForcedExitChecker;

#[async_trait::async_trait]
impl ForcedExitAccountAgeChecker for DummyForcedExitChecker {
    async fn check_forced_exit(
        &self,
        _storage: &mut StorageProcessor<'_>,
        _target_account_address: Address,
    ) -> Result<bool, SubmitError> {
        Ok(true)
    }

    async fn validate_forced_exit(
        &self,
        _storage: &mut StorageProcessor<'_>,
        _target_account_address: Address,
    ) -> Result<(), SubmitError> {
        Ok(())
    }
}
