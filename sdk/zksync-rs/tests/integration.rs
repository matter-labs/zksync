//! Integration test for zkSync Rust SDK.
//!
//! In order to pass these tests, there must be a running
//! instance of zkSync server and prover:
//!
//! ```bash
//! zksync server &!
//! zksync dummy-prover &!
//! zksync sdk-test
//! ```

use futures::compat::Future01CompatExt;
use std::time::{Duration, Instant};
use zksync::{
    web3::types::{H160, H256, U256},
    zksync_models::node::tx::PackedEthSignature,
    EthereumProvider, Network, Provider, Wallet, WalletCredentials,
};

const ETH_ADDR: &str = "36615Cf349d7F6344891B1e7CA7C72883F5dc049";
const ETH_PRIVATE_KEY: &str = "7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110";
const LOCALHOST_WEB3_ADDR: &str = "http://127.0.0.1:8545";

fn eth_main_account_credentials() -> (H160, H256) {
    let addr = ETH_ADDR.parse().unwrap();
    let eth_private_key = ETH_PRIVATE_KEY.parse().unwrap();

    (addr, eth_private_key)
}

fn eth_random_account_credentials() -> (H160, H256) {
    // let addr = ETH_ADDR.parse().unwrap();
    let mut eth_private_key = H256::default();
    eth_private_key.randomize();

    let address_from_pk = PackedEthSignature::address_from_private_key(&eth_private_key).unwrap();

    (address_from_pk, eth_private_key)
}

fn one_ether() -> U256 {
    U256::from(10).pow(18.into())
}

async fn wait_for_eth_tx(ethereum: &EthereumProvider, hash: H256) {
    let timeout = Duration::from_secs(10);
    let mut poller = tokio::time::interval(std::time::Duration::from_millis(100));
    let web3 = ethereum.web3();
    let start = Instant::now();
    while web3
        .eth()
        .transaction_receipt(hash)
        .compat()
        .await
        .unwrap()
        .is_none()
    {
        if start.elapsed() > timeout {
            panic!("Timeout elapsed while waiting for Ethereum transaction");
        }
        poller.tick().await;
    }
}

async fn wait_for_deposit_and_update_account_id(wallet: &mut Wallet) {
    let timeout = Duration::from_secs(60);
    let mut poller = tokio::time::interval(std::time::Duration::from_millis(100));
    let start = Instant::now();
    while wallet
        .provider
        .account_info(wallet.address())
        .await
        .unwrap()
        .id
        .is_none()
    {
        if start.elapsed() > timeout {
            panic!("Timeout elapsed while waiting for Ethereum transaction");
        }
        poller.tick().await;
    }

    wallet.update_account_id().await.unwrap();
    assert!(wallet.account_id().is_some(), "Account ID was not set");
}

async fn transfer_eth_to(to: H160) {
    let (main_eth_address, main_eth_private_key) = eth_main_account_credentials();

    let provider = Provider::new(Network::Localhost);
    let credentials =
        WalletCredentials::from_eth_pk(main_eth_address, main_eth_private_key, 1337).unwrap();

    let wallet = Wallet::new(provider, credentials).await.unwrap();
    let ethereum = wallet.ethereum(LOCALHOST_WEB3_ADDR).await.unwrap();

    let hash = ethereum.transfer("ETH", one_ether(), to).await.unwrap();

    wait_for_eth_tx(&ethereum, hash).await;
}

#[tokio::test]
#[cfg_attr(not(feature = "integration-tests"), ignore)]
async fn simple_workflow() -> Result<(), anyhow::Error> {
    let (eth_address, eth_private_key) = eth_random_account_credentials();

    // Transfer funds from "rich" account to a randomly created one (so we won't reuse the same
    // account in subsequent test runs).
    transfer_eth_to(eth_address).await;

    let provider = Provider::new(Network::Localhost);
    let credentials = WalletCredentials::from_eth_pk(eth_address, eth_private_key, 1337).unwrap();

    let mut wallet = Wallet::new(provider, credentials).await.unwrap();
    let ethereum = wallet.ethereum(LOCALHOST_WEB3_ADDR).await.unwrap();

    let deposit_tx_hash = ethereum
        .deposit("ETH", one_ether() / 2, wallet.address())
        .await
        .unwrap();

    wait_for_eth_tx(&ethereum, deposit_tx_hash).await;

    // Update stored wallet ID after we initialized a wallet via deposit.
    wait_for_deposit_and_update_account_id(&mut wallet).await;

    if !wallet.is_signing_key_set().await.unwrap() {
        let handle = wallet.start_change_pubkey().send().await.unwrap();

        handle
            .commit_timeout(Duration::from_secs(60))
            .wait_for_commit()
            .await
            .unwrap();
    }

    // Perform transfer to self.
    let handle = wallet
        .start_transfer()
        .to(wallet.address())
        .token("ETH")
        .unwrap()
        .amount(1_000_000u64)
        .send()
        .await
        .unwrap();

    handle
        .verify_timeout(Duration::from_secs(180))
        .wait_for_verify()
        .await
        .unwrap();

    Ok(())
}
