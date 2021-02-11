use std::{convert::TryFrom, str::FromStr, time::Instant};

use anyhow::format_err;
use api::Accounts;
use ethabi::{Contract as ContractAbi, Hash};
use num::BigUint;
use std::fmt::Debug;
use std::time::Duration;
use tokio::task::JoinHandle;
use web3::{
    api,
    contract::{Contract, Options},
    transports::Http,
    types::{BlockNumber, FilterBuilder, Log},
    Web3,
};
use zksync_config::ZkSyncConfig;
use zksync_storage::{
    chain::operations_ext::records::TxReceiptResponse, ConnectionPool, StorageProcessor,
};

use zksync_api::core_api_client::CoreApiClient;

use zksync_types::{
    tx::{EthSignData, PackedEthSignature, TimeRange, TxEthSignature, TxHash, TxSignature},
    Account, AccountId, Address, PubKeyHash, ZkSyncTx, H256,
};

pub mod eth_watch;
pub mod forced_exit_sender;

use forced_exit_sender::ForcedExitSender;
use zksync_types::tx::ChangePubKey;
use zksync_types::SignedZkSyncTx;

use franklin_crypto::{
    alt_babyjubjub::fs::FsRepr,
    bellman::{pairing::bn256, PrimeFieldRepr},
};

use zksync_crypto::franklin_crypto::{eddsa::PrivateKey, jubjub::JubjubEngine};

pub type Engine = bn256::Bn256;

pub type Fr = bn256::Fr;
pub type Fs = <Engine as JubjubEngine>::Fs;
use zksync_crypto::ff::PrimeField;

use tokio::time;
use zksync_eth_signer::{EthereumSigner, PrivateKeySigner};

use zksync_types::Nonce;
use zksync_types::TokenId;

#[macro_use]
use vlog;

#[must_use]
pub fn run_forced_exit_requests_actors(
    pool: ConnectionPool,
    config: ZkSyncConfig,
) -> JoinHandle<()> {
    let core_api_client = CoreApiClient::new(config.api.private.url.clone());

    let eth_watch_handle =
        eth_watch::run_forced_exit_contract_watcher(core_api_client, pool, config);

    eth_watch_handle
}

// pub fn get_sk_from_hex(hex_string: String) -> PrivateKey<Engine> {

// }

// This private key is for testing purposes only and shoud not be used in production
// The address should be 0xe1faB3eFD74A77C23B426c302D96372140FF7d0C
const FORCED_EXIT_SENDER_ETH_PRIVATE_KEY: &str =
    "0x0559b9f000b4e4bbb7fe02e1374cef9623c2ab7c3791204b490e1f229191d104";

fn read_signing_key(private_key: &[u8]) -> anyhow::Result<PrivateKey<Engine>> {
    let mut fs_repr = FsRepr::default();
    fs_repr.read_be(private_key)?;
    Ok(PrivateKey::<Engine>(
        Fs::from_repr(fs_repr).expect("couldn't read private key from repr"),
    ))
}

pub async fn check_forced_exit_sender_prepared<'a>(
    storage: &mut StorageProcessor<'a>,
    sender_sk: &PrivateKey<Engine>,
    sender_address: Address,
) -> anyhow::Result<bool> {
    let mut accounts_schema = storage.chain().account_schema();

    let state = accounts_schema
        .account_state_by_address(sender_address)
        .await?
        .committed;

    match state {
        Some(account_state) => {
            let pk_hash = account_state.1.pub_key_hash;

            let sk_pub_key_hash = PubKeyHash::from_privkey(sender_sk);

            Ok(pk_hash == sk_pub_key_hash)
        }
        None => Ok(false),
    }
}

pub async fn wait_for_account_id<'a>(
    storage: &mut StorageProcessor<'a>,
    sender_address: Address,
) -> anyhow::Result<AccountId> {
    vlog::info!("Forced exit sender account is not yet prepared. Waiting for account id...");

    let mut account_schema = storage.chain().account_schema();
    let mut timer = time::interval(Duration::from_secs(1));

    loop {
        let account_id = account_schema.account_id_by_address(sender_address).await?;

        match account_id {
            Some(id) => {
                vlog::info!("Forced exit sender account has account id = {}", 1);
                return Ok(id);
            }
            None => {
                timer.tick().await;
            }
        }
    }
}

async fn get_receipt<'a>(
    storage: &mut StorageProcessor<'a>,
    tx_hash: TxHash,
) -> anyhow::Result<Option<TxReceiptResponse>> {
    storage
        .chain()
        .operations_ext_schema()
        .tx_receipt(tx_hash.as_ref())
        .await
}

pub async fn wait_for_change_pub_key_tx<'a>(
    storage: &mut StorageProcessor<'a>,
    tx_hash: TxHash,
) -> anyhow::Result<()> {
    vlog::info!(
        "Forced exit sender account is not yet prepared. Waiting for public key to be set..."
    );

    let mut timer = time::interval(Duration::from_secs(1));

    loop {
        let tx_receipt = get_receipt(storage, tx_hash)
            .await
            .expect("Faield t oget the traecipt pf ChangePubKey transaction");

        match tx_receipt {
            Some(receipt) => {
                if receipt.success {
                    vlog::info!("Public key of the forced exit sender successfully set");
                    return Ok(());
                } else {
                    let fail_reason = receipt.fail_reason.unwrap_or(String::from("unknown"));
                    panic!(
                        "Failed to set public for forced exit sedner. Reason: {}",
                        fail_reason
                    );
                }
            }
            None => {
                timer.tick().await;
            }
        }
    }
}

// Use PackedEthSignature::address_from_private_key
async fn get_verified_eth_sk(sender_address: Address) -> H256 {
    let eth_sk = hex::decode(&FORCED_EXIT_SENDER_ETH_PRIVATE_KEY[2..])
        .expect("Failed to parse eth signing key of the forced exit account");

    let private_key = H256::from_slice(&eth_sk);

    let pk_address = PackedEthSignature::address_from_private_key(&private_key).unwrap();

    dbg!(pk_address.clone());
    dbg!(sender_address.clone());

    if pk_address != sender_address {
        panic!("Private key provided does not correspond to the sender address");
    }

    private_key
}

pub async fn register_signing_key<'a>(
    storage: &mut StorageProcessor<'a>,
    sender_id: AccountId,
    api_client: CoreApiClient,
    sender_address: Address,
    sender_sk: &PrivateKey<Engine>,
) -> anyhow::Result<()> {
    let eth_sk = get_verified_eth_sk(sender_address).await;

    // Unfortunately, currently the only way to create a CPK
    // transaction from eth_private_key is to cre
    let mut cpk_tx = ChangePubKey::new_signed(
        sender_id,
        sender_address,
        PubKeyHash::from_privkey(sender_sk),
        TokenId::from_str("0").unwrap(),
        BigUint::from(0u8),
        Nonce::from_str("0").unwrap(),
        TimeRange::default(),
        None,
        sender_sk,
    )
    .expect("Failed to create unsigned cpk transaction");

    let eth_sign_bytes = cpk_tx
        .get_eth_signed_data()
        .expect("Failed to get eth signed data");

    let eth_signature =
        PackedEthSignature::sign(&eth_sk, &eth_sign_bytes).expect("Failed to sign eth message");

    cpk_tx.eth_signature = Some(PackedEthSignature::from(eth_signature.clone()));

    let tx = ZkSyncTx::ChangePubKey(Box::new(cpk_tx));
    let eth_sign_data = EthSignData {
        signature: TxEthSignature::EthereumSignature(eth_signature),
        message: eth_sign_bytes,
    };

    let tx_signed = SignedZkSyncTx {
        tx,
        eth_sign_data: Some(eth_sign_data),
    };
    let tx_hash = tx_signed.tx.hash();

    api_client
        .send_tx(tx_signed)
        .await
        .expect("Failed to send CPK transaction");

    wait_for_change_pub_key_tx(storage, tx_hash)
        .await
        .expect("Failed to wait for ChangePubKey tx");

    Ok(())
}

fn verify_pub_key_hash() {}

pub async fn prepare_forced_exit_sender(
    connection_pool: ConnectionPool,
    api_client: CoreApiClient,
    config: &ZkSyncConfig,
) -> anyhow::Result<()> {
    let mut storage = connection_pool
        .access_storage()
        .await
        .expect("forced_exit_requests: Failed to get the connection to storage");

    let sender_sk = hex::decode(&config.forced_exit_requests.sender_private_key[2..])
        .expect("Failed to decode forced_exit_sender sk");
    let sender_sk = read_signing_key(&sender_sk).expect("Failed to read forced exit sender sk");
    let sender_address = config.forced_exit_requests.sender_account_address;

    let is_sender_prepared =
        check_forced_exit_sender_prepared(&mut storage, &sender_sk, sender_address)
            .await
            .expect("Failed to check if the sender is prepared");

    if is_sender_prepared {
        return Ok(());
    }

    // The sender is not prepared. This should not ever happen in production, but handling
    // such step is vital for testing locally.

    // Waiting until the sender has an id (sending funds to the account should be done by an external script)
    let id = wait_for_account_id(&mut storage, sender_address)
        .await
        .expect("Failed to get account id for forced exit sender");

    register_signing_key(&mut storage, id, api_client, sender_address, &sender_sk).await?;

    Ok(())
}

// Inserts the forced exit sender account into db
// should be used only for local setup/testing
// pub fn insert_forced_exit_account(config: &ZkSyncConfig) {
//     let pool = ConnectionPool::new(Some(1));

//     vlog::info!("Inserting forced exit sender into db");

//     let sender_account = Account::
// }

/*

Polling like eth_watch

If sees a funds_received -> extracts id

Get_by_id => gets by id

If sum is enough => set_fullfilled_and_send_tx


FE requests consist of 2 (or 3 if needed actors)


**/
