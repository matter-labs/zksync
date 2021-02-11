use std::{convert::TryFrom, time::Instant};

use anyhow::format_err;
use ethabi::{Contract as ContractAbi, Hash};
use fee_ticker::validator::watcher;
use franklin_crypto::bellman::PrimeFieldRepr;
use num::{BigUint, FromPrimitive, ToPrimitive};
use std::fmt::Debug;
use tokio::task::JoinHandle;
use web3::{
    contract::{Contract, Options},
    transports::Http,
    types::{BlockNumber, FilterBuilder, Log},
    Web3,
};
use zksync_config::ZkSyncConfig;
use zksync_storage::{
    chain::{account::AccountSchema, operations_ext::records::TxReceiptResponse},
    ConnectionPool, StorageProcessor,
};

use zksync_contracts::zksync_contract;
use zksync_types::{
    forced_exit_requests::ForcedExitRequest, tx::TimeRange, tx::TxHash, AccountId, Address, Nonce,
    PriorityOp, TokenId, ZkSyncTx, H160, U256,
};

use chrono::Utc;
use zksync_api::{api_server::rpc_server, core_api_client::CoreApiClient, fee_ticker};
use zksync_core::eth_watch::get_contract_events;
use zksync_types::forced_exit_requests::FundsReceivedEvent;
use zksync_types::ForcedExit;
use zksync_types::SignedZkSyncTx;

use super::PrivateKey;
use super::{Engine, Fs, FsRepr};

use zksync_crypto::ff::PrimeField;

use crate::eth_watch;

pub struct ForcedExitSender {
    core_api_client: CoreApiClient,
    connection_pool: ConnectionPool,
    config: ZkSyncConfig,
    operator_account_id: AccountId,
    sender_private_key: PrivateKey<Engine>,
}
async fn get_operator_account_id(
    connection_pool: ConnectionPool,
    config: &ZkSyncConfig,
) -> anyhow::Result<AccountId> {
    let mut storage = connection_pool.access_storage().await?;
    let mut accounts_schema = storage.chain().account_schema();

    let account_id = accounts_schema
        .account_id_by_address(config.forced_exit_requests.sender_account_address)
        .await?;

    account_id.ok_or(anyhow::Error::msg("1"))
}

// A dummy tmp function
fn send_to_mempool(account_id: AccountId, token: TokenId) {
    let msg = format!(
        "The following tx was sent to mempool {} {}",
        account_id, token
    );
    dbg!(msg);
}

fn read_signing_key(private_key: &[u8]) -> anyhow::Result<PrivateKey<Engine>> {
    let mut fs_repr = FsRepr::default();
    fs_repr.read_be(private_key)?;
    Ok(PrivateKey::<Engine>(
        Fs::from_repr(fs_repr).expect("couldn't read private key from repr"),
    ))
}

impl ForcedExitSender {
    pub async fn new(
        core_api_client: CoreApiClient,
        connection_pool: ConnectionPool,
        config: ZkSyncConfig,
    ) -> anyhow::Result<Self> {
        let operator_account_id = get_operator_account_id(connection_pool.clone(), &config)
            .await
            .expect("Failed to get the sender id");

        let sender_private_key =
            hex::decode(&config.clone().forced_exit_requests.sender_private_key[2..])
                .expect("Decoding private key failed");
        let sender_private_key =
            read_signing_key(&sender_private_key).expect("Reading private key failed");

        Ok(Self {
            core_api_client,
            connection_pool,
            operator_account_id,
            config,
            sender_private_key,
        })
    }

    pub fn extract_id_from_amount(&self, amount: i64) -> i64 {
        let id_space_size: i64 =
            (10 as i64).pow(self.config.forced_exit_requests.digits_in_id as u32);

        amount % id_space_size
    }

    pub async fn construct_forced_exit<'a>(
        &self,
        storage: &mut StorageProcessor<'a>,
        fe_request: ForcedExitRequest,
    ) -> anyhow::Result<SignedZkSyncTx> {
        let mut account_schema = storage.chain().account_schema();

        let operator_state = account_schema
            .last_committed_state_for_account(self.operator_account_id)
            .await?
            .expect("The operator account has no committed state");
        let operator_nonce = operator_state.nonce;

        // TODO: allow batches
        let tx = ForcedExit::new_signed(
            self.operator_account_id,
            fe_request.target,
            fe_request.tokens[0],
            BigUint::from(0u32),
            operator_nonce,
            TimeRange::default(),
            &self.sender_private_key,
        )
        .expect("Failed to create signed transaction from ForcedExit");

        Ok(SignedZkSyncTx {
            tx: ZkSyncTx::ForcedExit(Box::new(tx)),
            eth_sign_data: None,
        })
    }

    // TODO: take the block timestamp into account instead of
    // the now
    pub fn expired(&self, request: &ForcedExitRequest) -> bool {
        let now_millis = Utc::now().timestamp_millis();
        let created_at_millis = request.created_at.timestamp_millis();

        return now_millis.saturating_sub(created_at_millis)
            <= self.config.forced_exit_requests.max_tx_interval;
    }

    // Returns the id the request if it should be fulfilled,
    // error otherwise
    pub fn verify_request(
        &self,
        amount: i64,
        request: Option<ForcedExitRequest>,
    ) -> anyhow::Result<ForcedExitRequest> {
        let request = match request {
            Some(r) => r,
            None => {
                return Err(anyhow::Error::msg("The request was not found"));
            }
        };

        if self.expired(&request) {
            return Err(anyhow::Error::msg("The request was not found"));
        }

        if request.price_in_wei != BigUint::from_i64(amount).unwrap() {
            return Err(anyhow::Error::msg("The request was not found"));
        }

        return Ok(request);
    }

    pub async fn get_request_by_id<'a>(
        &self,
        storage: &mut StorageProcessor<'a>,
        id: i64,
    ) -> anyhow::Result<Option<ForcedExitRequest>> {
        let mut fe_schema = storage.forced_exit_requests_schema();

        let request = fe_schema.get_request_by_id(id).await?;
        // {
        //     Ok(r) => r,
        //     Err(e) => {
        //         log::warn!("ForcedExitRequests: Fail to get request by id: {}", e);
        //         return;
        //     }
        // };

        Ok(request)
    }

    pub async fn fulfill_request<'a>(
        &self,
        storage: &mut StorageProcessor<'a>,
        id: i64,
    ) -> anyhow::Result<()> {
        let mut fe_schema = storage.forced_exit_requests_schema();

        fe_schema
            .fulfill_request(id, Utc::now())
            .await
            // TODO: Handle such cases gracefully, and not panic
            .expect("An error occured, while fu;lfilling the request");

        log::info!("FE request with id {} was fulfilled", id);

        Ok(())
    }

    pub async fn get_receipt<'a>(
        &self,
        storage: &mut StorageProcessor<'a>,
        tx_hash: TxHash,
    ) -> anyhow::Result<Option<TxReceiptResponse>> {
        storage
            .chain()
            .operations_ext_schema()
            .tx_receipt(tx_hash.as_ref())
            .await
    }

    pub async fn wait_until_comitted<'a>(
        &self,
        storage: &mut StorageProcessor<'a>,
        tx_hash: TxHash,
    ) -> anyhow::Result<()> {
        let poll_interval: i32 = 200;

        // If there is no receipt for 20 seconds, we consider the comitment failed
        let timeout: i32 = 60000;
        let mut time_passed: i32 = 0;

        loop {
            if time_passed >= timeout {
                panic!("Comitting tx failed!");
            }

            let receipt = self.get_receipt(storage, tx_hash).await?;

            if let Some(tx_receipt) = receipt {
                if tx_receipt.success {
                    return Ok(());
                } else {
                    panic!("FE Transaction failed")
                }
            }
        }
    }

    pub async fn process_request(&self, amount: i64) {
        let id = self.extract_id_from_amount(amount);

        let mut storage = match self.connection_pool.access_storage().await {
            Ok(storage) => storage,
            Err(error) => {
                log::warn!("Failed to acquire db connection for processing forced_exit_request, reason: {}", error);
                return;
            }
        };

        let fe_request = self
            .get_request_by_id(&mut storage, id)
            .await
            .expect("Failed to get request by id");

        let fe_request = match self.verify_request(amount, fe_request) {
            Ok(r) => r,
            Err(_) => {
                // The request was not valid, that's fine
                return;
            }
        };

        let fe_tx = self
            .construct_forced_exit(&mut storage, fe_request)
            .await
            .expect("Failed to construct ForcedExit");
        let tx_hash = fe_tx.hash();

        self.core_api_client
            .send_tx(fe_tx)
            .await
            .expect("Failed to send transaction to mempool")
            .unwrap();

        self.wait_until_comitted(&mut storage, tx_hash)
            .await
            .expect("Comittment waiting failed");

        self.fulfill_request(&mut storage, id)
            .await
            .expect("Error while fulfulling the request");

        // let db_transaction = match fe_schema.0.start_transaction().await {
        //     Ok(transaction ) => transaction,
        //     Err(error) => {
        //         log::warn!("Failed to start db transaction for processing forced_exit_requests, reason {}", error);
        //         return;
        //     }
        // };

        // send_to_mempool();
        // await until its committed

        //  db_transaction
        //      .fe_schema()
        //    .fulfill_request()

        // let fe_request = fe_schema.get_request_by_id(id).await;
        // // The error means that such on id does not exists
        // // TOOD: Actually handle differently when id does not exist or an actual error
        // if let Err(_) = fe_request {
        //     return;
        // }

        // let fe_request = fe_request.unwrap().unwrap();

        // TODO: take aging into account

        // let tx = self.construct_forced_exit(storage, fe_request).await.expect("Failed to construct forced exit transaction");
        // TODO: Handle such cases gracefully, and not panic
        // self.core_api_client.send_tx(tx).await.expect("An erro occureed, while submitting tx");
    }
}
