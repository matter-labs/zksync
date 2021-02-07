use std::{convert::TryFrom, time::Instant};

use anyhow::format_err;
use ethabi::{Contract as ContractAbi, Hash};
use num::{BigUint, ToPrimitive};
use std::fmt::Debug;
use tokio::task::JoinHandle;
use web3::{
    contract::{Contract, Options},
    transports::Http,
    types::{BlockNumber, FilterBuilder, Log},
    Web3,
};
use zksync_config::ZkSyncConfig;
use zksync_storage::{ConnectionPool, StorageProcessor};

use zksync_contracts::zksync_contract;
use zksync_types::{
    forced_exit_requests::ForcedExitRequest, tx::TimeRange, AccountId, Address, Nonce, PriorityOp,
    ZkSyncTx, H160, U256,
};

use chrono::Utc;
use zksync_api::{core_api_client::CoreApiClient, fee_ticker};
use zksync_core::eth_watch::get_contract_events;
use zksync_types::forced_exit_requests::FundsReceivedEvent;
use zksync_types::ForcedExit;
use zksync_types::SignedZkSyncTx;

use crate::eth_watch;

pub struct ForcedExitSender {
    core_api_client: CoreApiClient,
    connection_pool: ConnectionPool,
    config: ZkSyncConfig,
    operator_account_id: AccountId,
}
async fn get_operator_account_id(
    connection_pool: ConnectionPool,
    config: &ZkSyncConfig,
) -> anyhow::Result<AccountId> {
    let mut storage = connection_pool.access_storage().await?;
    let mut accounts_schema = storage.chain().account_schema();

    let account_id = accounts_schema
        .account_id_by_address(config.eth_sender.sender.operator_commit_eth_addr)
        .await?;

    account_id.ok_or(anyhow::Error::msg("1"))
}

impl ForcedExitSender {
    pub async fn new(
        core_api_client: CoreApiClient,
        connection_pool: ConnectionPool,
        config: ZkSyncConfig,
    ) -> anyhow::Result<Self> {
        let operator_account_id = get_operator_account_id(connection_pool.clone(), &config).await?;

        Ok(Self {
            core_api_client,
            connection_pool,
            operator_account_id,
            config,
        })
    }

    pub fn extract_id_from_amount(&self, amount: i64) -> i64 {
        let id_space_size: i64 =
            (10 as i64).pow(self.config.forced_exit_requests.digits_in_id as u32);

        amount % id_space_size
    }

    // pub async fn construct_forced_exit<'a>(
    //     &self,
    //     storage: StorageProcessor<'a>,
    //     fe_request: ForcedExitRequest
    // ) -> anyhow::Result<SignedZkSyncTx> {

    //     let account_schema = storage.chain().account_schema();

    //     let operator_state = account_schema.last_committed_state_for_account(self.operator_account_id).await?.expect("The operator account has no committed state");
    //     let operator_nonce = operator_state.nonce;

    //     // TODO: allow batches
    //     let tx = ForcedExit::new_signed(
    //         self.operator_account_id,
    //         fe_request.target,
    //         fe_request.tokens[0],
    //         BigUint::from(0),
    //         operator_nonce,
    //         TimeRange::default(),
    //         self.config.eth_sender.sender.operator_private_key.clone(),
    //     ).expect("Failed to create signed transaction from ForcedExit");

    //     Ok(SignedZkSyncTx {
    //         tx: ZkSyncTx::ForcedExit(Box::new(tx)),
    //         eth_sign_data: None
    //     })
    // }

    pub async fn process_request(&self, amount: i64) {
        let id = self.extract_id_from_amount(amount);

        let mut storage = self
            .connection_pool
            .access_storage()
            .await
            .expect("forced_exit_porcess_request");

        let mut fe_schema = storage.forced_exit_requests_schema();

        let fe_request = fe_schema.get_request_by_id(id).await;
        // The error means that such on id does not exists
        // TOOD: Actually handle differently when id does not exist or an actual error
        if let Err(_) = fe_request {
            return;
        }

        let fe_request = fe_request.unwrap();

        // TODO: take aging into account
        if fe_request.id == id && fe_request.price_in_wei.to_i64().unwrap() == amount {
            fe_schema
                .fulfill_request(id, Utc::now())
                .await
                // TODO: Handle such cases gracefully, and not panic
                .expect("An error occured, while fu;lfilling the request");

            log::info!("FE request with id {} was fulfilled", id);

            // let tx = self.construct_forced_exit(storage, fe_request).await.expect("Failed to construct forced exit transaction");
            // TODO: Handle such cases gracefully, and not panic
            // self.core_api_client.send_tx(tx).await.expect("An erro occureed, while submitting tx");
        }
    }
}
