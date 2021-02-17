use num::BigUint;
use std::str::FromStr;
use std::time::Duration;
use tokio::task::JoinHandle;
use zksync_config::ZkSyncConfig;
use zksync_storage::{
    chain::operations_ext::records::TxReceiptResponse, ConnectionPool, StorageProcessor,
};

use zksync_api::core_api_client::CoreApiClient;

use zksync_types::{
    tx::{EthSignData, PackedEthSignature, TimeRange, TxEthSignature, TxHash},
    AccountId, Address, PubKeyHash, ZkSyncTx, H256,
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

use zksync_types::Nonce;
use zksync_types::TokenId;

#[must_use]
pub fn run_forced_exit_requests_actors(
    pool: ConnectionPool,
    config: ZkSyncConfig,
) -> JoinHandle<()> {
    let core_api_client = CoreApiClient::new(config.api.private.url.clone());
    eth_watch::run_forced_exit_contract_watcher(core_api_client, pool, config)
}

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

pub async fn check_forced_exit_sender_prepared(
    storage: &mut StorageProcessor<'_>,
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

pub async fn wait_for_account_id(
    storage: &mut StorageProcessor<'_>,
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

async fn get_receipt(
    storage: &mut StorageProcessor<'_>,
    tx_hash: TxHash,
) -> anyhow::Result<Option<TxReceiptResponse>> {
    storage
        .chain()
        .operations_ext_schema()
        .tx_receipt(tx_hash.as_ref())
        .await
}

pub async fn wait_for_change_pub_key_tx(
    storage: &mut StorageProcessor<'_>,
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
                    let fail_reason = receipt
                        .fail_reason
                        .unwrap_or_else(|| String::from("unknown"));
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

    if pk_address != sender_address {
        panic!("Private key provided does not correspond to the sender address");
    }

    private_key
}

pub async fn register_signing_key(
    storage: &mut StorageProcessor<'_>,
    sender_id: AccountId,
    api_client: CoreApiClient,
    sender_address: Address,
    sender_sk: &PrivateKey<Engine>,
) -> anyhow::Result<()> {
    let eth_sk = get_verified_eth_sk(sender_address).await;

    let pub_key_hash = PubKeyHash::from_privkey(sender_sk);

    // Unfortunately, currently the only way to create a CPK
    // transaction from eth_private_key is to cre
    let cpk_tx = ChangePubKey::new_signed(
        sender_id,
        sender_address,
        pub_key_hash,
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

    let cpk_tx_signed = ChangePubKey::new_signed(
        sender_id,
        sender_address,
        pub_key_hash,
        TokenId::from_str("0").unwrap(),
        BigUint::from(0u8),
        Nonce::from_str("0").unwrap(),
        TimeRange::default(),
        Some(eth_signature.clone()),
        sender_sk,
    )
    .expect("Failed to created signed CPK transaction");

    let tx = ZkSyncTx::ChangePubKey(Box::new(cpk_tx_signed));
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
        .expect("Failed to send CPK transaction")
        .expect("Failed to send");

    wait_for_change_pub_key_tx(storage, tx_hash)
        .await
        .expect("Failed to wait for ChangePubKey tx");

    Ok(())
}

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
