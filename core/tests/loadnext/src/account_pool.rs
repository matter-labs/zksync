use std::{
    collections::VecDeque,
    str::FromStr,
    sync::{Arc, RwLock},
};

use rand::{thread_rng, Rng};

use zksync::{utils::private_key_from_seed, RpcProvider, Wallet, WalletCredentials};
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::{tx::PackedEthSignature, Address, H256};

use crate::config::LoadtestConfig;

/// Thread-safe pool of the addresses of accounts used in the loadtest.
///
/// Sync std `RwLock` is chosen instead of async `tokio` one because of `rand::thread_rng()` usage:
/// since it's not `Send`, using it in the async functions will make all the affected futures also not
/// `Send`, which in its turn will make it impossible to be used in `tokio::spawn`.
///
/// As long as we only use `read` operation on the lock, it doesn't really matter which kind of lock we use.
#[derive(Debug, Clone)]
pub struct AddressPool {
    pub addresses: Arc<RwLock<Vec<Address>>>,
}

impl AddressPool {
    pub fn new(addresses: Vec<Address>) -> Self {
        Self {
            addresses: Arc::new(RwLock::new(addresses)),
        }
    }

    /// Randomly chooses on of the addresses stored in the pool.
    pub fn random_address(&self) -> Address {
        let rng = &mut thread_rng();

        let addresses = self.addresses.read().unwrap();
        let index = rng.gen_range(0, addresses.len());

        addresses[index]
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

impl AccountCredentials {
    /// Generates random credentials.
    pub fn rand() -> Self {
        let eth_pk = H256::random();
        let address = pk_to_address(&eth_pk);

        Self { eth_pk, address }
    }
}

/// Tuple that consists of pre-initialized wallet and the Ethereum private key.
/// We have to collect private keys, since `Wallet` doesn't expose it, and we may need it to resign transactions
/// (for example, if we want to create a corrupted transaction: `zksync` library won't allow us to do it, thus
/// we will have to sign such a transaction manually).
pub type TestWallet = (Wallet<PrivateKeySigner, RpcProvider>, H256);

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
    pub async fn new(config: &LoadtestConfig) -> Self {
        let provider = RpcProvider::from_addr_and_network(
            &config.zksync_rpc_addr,
            zksync::Network::from_str(&config.eth_network).expect("Invalid network name"),
        );

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

        let mut accounts = VecDeque::new();
        let mut addresses = Vec::new();

        for _ in 0..config.accounts_amount {
            let eth_credentials = AccountCredentials::rand();
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
            accounts.push_back((wallet, eth_credentials.eth_pk));
        }

        Self {
            master_wallet,
            accounts,
            addresses: AddressPool::new(addresses),
        }
    }
}

fn pk_to_address(eth_pk: &H256) -> Address {
    PackedEthSignature::address_from_private_key(&eth_pk)
        .expect("Can't get an address from the private key")
}
