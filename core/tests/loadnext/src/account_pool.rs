use std::{collections::VecDeque, str::FromStr, sync::Arc, time::Duration};

use rand::Rng;

use tokio::time::timeout;
use zksync::{
    provider::Provider, utils::private_key_from_seed, RpcProvider, Wallet, WalletCredentials,
};
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::{tx::PackedEthSignature, Address, H256};

use crate::{
    config::LoadtestConfig,
    rng::{LoadtestRng, Random},
};

/// Thread-safe pool of the addresses of accounts used in the loadtest.
#[derive(Debug, Clone)]
pub struct AddressPool {
    addresses: Arc<Vec<Address>>,
}

impl AddressPool {
    pub fn new(addresses: Vec<Address>) -> Self {
        Self {
            addresses: Arc::new(addresses),
        }
    }

    /// Randomly chooses one of the addresses stored in the pool.
    pub fn random_address(&self, rng: &mut LoadtestRng) -> Address {
        let index = rng.gen_range(0, self.addresses.len());
        self.addresses[index]
    }
}

/// Credentials for a test account.
/// Currently we support only EOA accounts.
#[derive(Debug, Clone)]
pub struct AccountCredentials {
    /// Ethereum private key.
    pub eth_pk: H256,
    /// Ethereum address derived from the private key.
    pub address: Address,
}

impl Random for AccountCredentials {
    fn random(rng: &mut LoadtestRng) -> Self {
        let eth_pk = H256::random_using(rng);
        let address = pk_to_address(&eth_pk);

        Self { eth_pk, address }
    }
}
/// Type that contains the data required for the test wallet to operate.
#[derive(Debug)]
pub struct TestWallet {
    /// Pre-initialized wallet object.
    pub wallet: Wallet<PrivateKeySigner, RpcProvider>,
    /// Ethereum private key of the wallet.
    /// We have to collect private keys, since `Wallet` doesn't expose it, and we may need it to resign transactions
    /// (for example, if we want to create a corrupted transaction: `zksync` library won't allow us to do it, thus
    /// we will have to sign such a transaction manually).
    /// zkSync private key can be restored from the Ethereum one using `private_key_from_seed` function.
    pub eth_pk: H256,
    /// RNG object derived from a common loadtest seed and the wallet private key.
    pub rng: LoadtestRng,
}

/// Pool of accounts to be used in the test.
/// Each account is represented as `zksync::Wallet` in order to provide convenient interface of interation with zkSync.
#[derive(Debug)]
pub struct AccountPool {
    /// Main wallet that will be used to initialize all the test wallets.
    pub master_wallet: Wallet<PrivateKeySigner, RpcProvider>,
    /// Collection of test wallets and their Ethereum private keys.
    pub accounts: VecDeque<TestWallet>,
    /// Pool of addresses of the test accounts.
    pub addresses: AddressPool,
}

impl AccountPool {
    /// Generates all the required test accounts and prepares `Wallet` objects.
    pub async fn new(config: &LoadtestConfig) -> anyhow::Result<Self> {
        let provider = RpcProvider::from_addr_and_network(
            &config.zksync_rpc_addr,
            zksync::Network::from_str(&config.eth_network).expect("Invalid network name"),
        );

        // Perform a health check: check whether zkSync server is alive.
        let mut server_alive = false;
        for _ in 0usize..3 {
            if let Ok(Ok(_)) = timeout(Duration::from_secs(3), provider.contract_address()).await {
                server_alive = true;
                break;
            }
        }
        if !server_alive {
            anyhow::bail!("zkSync server does not respond. Please check RPC address and whether server is launched");
        }

        let mut rng = LoadtestRng::new_generic(config.seed.clone());
        vlog::info!("Using RNG with master seed: {}", rng.seed_hex());

        let master_wallet = {
            let eth_pk = H256::from_str(&config.master_wallet_pk)
                .expect("Can't parse master wallet private key");
            let address = pk_to_address(&eth_pk);
            let zksync_pk = private_key_from_seed(eth_pk.as_bytes())
                .expect("Can't generate the zkSync private key");
            let wallet_credentials =
                WalletCredentials::<PrivateKeySigner>::from_pk(address, zksync_pk, Some(eth_pk));
            Wallet::new(provider.clone(), wallet_credentials)
                .await
                .expect("Can't create a wallet")
        };

        let mut accounts = VecDeque::with_capacity(config.accounts_amount);
        let mut addresses = Vec::with_capacity(config.accounts_amount);

        for _ in 0..config.accounts_amount {
            let eth_credentials = AccountCredentials::random(&mut rng);
            let zksync_pk = private_key_from_seed(eth_credentials.eth_pk.as_bytes())
                .expect("Can't generate the zkSync private key");
            let wallet_credentials = WalletCredentials::<PrivateKeySigner>::from_pk(
                eth_credentials.address,
                zksync_pk,
                Some(eth_credentials.eth_pk),
            );

            let wallet = Wallet::new(provider.clone(), wallet_credentials)
                .await
                .expect("Can't create a wallet");

            addresses.push(wallet.address());
            let account = TestWallet {
                wallet,
                eth_pk: eth_credentials.eth_pk,
                rng: rng.derive(eth_credentials.eth_pk),
            };
            accounts.push_back(account);
        }

        Ok(Self {
            master_wallet,
            accounts,
            addresses: AddressPool::new(addresses),
        })
    }
}

fn pk_to_address(eth_pk: &H256) -> Address {
    PackedEthSignature::address_from_private_key(&eth_pk)
        .expect("Can't get an address from the private key")
}
