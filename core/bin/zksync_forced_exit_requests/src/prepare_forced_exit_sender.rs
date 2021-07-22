use num::BigUint;
use std::time::Duration;
use zksync_config::ZkSyncConfig;
use zksync_storage::{
    chain::operations_ext::records::TxReceiptResponse, ConnectionPool, StorageProcessor,
};

use zksync_api::core_api_client::CoreApiClient;
use zksync_types::{
    tx::{ChangePubKeyType, TimeRange, TxHash},
    AccountId, Address, PubKeyHash, ZkSyncTx, H256,
};

use zksync_types::{Nonce, SignedZkSyncTx, TokenId};

use zksync_crypto::franklin_crypto::eddsa::PrivateKey;

use tokio::time;

use zksync_test_account::{ZkSyncAccount, ZkSyncETHAccountData};

use super::utils::{read_signing_key, Engine};

pub async fn prepare_forced_exit_sender_account(
    connection_pool: ConnectionPool,
    api_client: CoreApiClient,
    config: &ZkSyncConfig,
) -> anyhow::Result<AccountId> {
    let mut storage = connection_pool
        .access_storage()
        .await
        .expect("forced_exit_requests: Failed to get the connection to storage");

    let sender_sk = hex::decode(&config.forced_exit_requests.sender_private_key[2..])
        .expect("Failed to decode forced_exit_sender sk");
    let sender_sk = read_signing_key(&sender_sk).expect("Failed to read forced exit sender sk");
    let sender_address = config.forced_exit_requests.sender_account_address;
    let sender_eth_private_key = config.forced_exit_requests.sender_eth_private_key;

    let is_sender_prepared =
        check_forced_exit_sender_prepared(&mut storage, &sender_sk, sender_address)
            .await
            .expect("Failed to check if the sender is prepared");

    if let Some(id) = is_sender_prepared {
        return Ok(id);
    }

    // The sender is not prepared. This should not ever happen in production, but handling
    // such step is vital for testing locally.

    // Waiting until the sender has an id (sending funds to the account should be done by an external script)
    let id = wait_for_account_id(&mut storage, sender_address)
        .await
        .expect("Failed to get account id for forced exit sender");

    register_signing_key(
        &mut storage,
        id,
        api_client,
        sender_address,
        sender_eth_private_key,
        sender_sk,
    )
    .await?;

    Ok(id)
}

pub async fn check_forced_exit_sender_prepared(
    storage: &mut StorageProcessor<'_>,
    sender_sk: &PrivateKey<Engine>,
    sender_address: Address,
) -> anyhow::Result<Option<AccountId>> {
    let mut accounts_schema = storage.chain().account_schema();

    let state = accounts_schema
        .account_state_by_address(sender_address)
        .await?
        .committed;

    match state {
        Some(account_state) => {
            let pk_hash = account_state.1.pub_key_hash;

            let sk_pub_key_hash = PubKeyHash::from_privkey(sender_sk);

            if pk_hash == sk_pub_key_hash {
                Ok(Some(account_state.0))
            } else {
                Ok(None)
            }
        }
        None => Ok(None),
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
                        "Failed to set public key for forced exit sedner. Reason: {}",
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

pub async fn register_signing_key(
    storage: &mut StorageProcessor<'_>,
    sender_id: AccountId,
    api_client: CoreApiClient,
    sender_address: Address,
    sender_eth_private_key: H256,
    sender_sk: PrivateKey<Engine>,
) -> anyhow::Result<()> {
    let eth_account_data = ZkSyncETHAccountData::EOA {
        eth_private_key: sender_eth_private_key,
    };

    let sender_account = ZkSyncAccount::new(
        sender_sk,
        // The account is changing public key for the first time, so nonce is 0
        Nonce(0),
        sender_address,
        eth_account_data,
    );
    sender_account.set_account_id(Some(sender_id));

    let cpk = sender_account.sign_change_pubkey_tx(
        Some(Nonce(0)),
        true,
        TokenId(0),
        BigUint::from(0u8),
        ChangePubKeyType::ECDSA,
        TimeRange::default(),
    );

    let tx = ZkSyncTx::ChangePubKey(Box::new(cpk));
    let tx_hash = tx.hash();

    api_client
        .send_tx(SignedZkSyncTx {
            tx,
            eth_sign_data: None,
        })
        .await
        .expect("Failed to send CPK transaction")
        .expect("Failed to send");

    wait_for_change_pub_key_tx(storage, tx_hash)
        .await
        .expect("Failed to wait for ChangePubKey tx");

    Ok(())
}
