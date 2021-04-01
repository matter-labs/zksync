use std::{collections::VecDeque, str::FromStr};

use zksync::{utils::private_key_from_seed, RpcProvider, Wallet, WalletCredentials};
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::{tx::PackedEthSignature, Address, H256};

use crate::config::LoadtestConfig;

/// Credentials for a test account.
/// Currently we support only EOA accounts.
#[derive(Debug, Clone)]
pub struct AccountCredentials {
    pub eth_pk: H256,
    pub address: Address,
}

impl AccountCredentials {
    pub fn rand() -> Self {
        let eth_pk = H256::random();
        let address = pk_to_address(&eth_pk);

        Self { eth_pk, address }
    }
}

#[derive(Debug)]
pub struct AccountPool {
    pub master_wallet: Wallet<PrivateKeySigner, RpcProvider>,
    pub accounts: VecDeque<Wallet<PrivateKeySigner, RpcProvider>>,
    pub addresses: Vec<Address>,
}

impl AccountPool {
    pub async fn new(config: &LoadtestConfig) -> Self {
        let provider =
            RpcProvider::from_addr_and_network(&config.zksync_rpc_addr, zksync::Network::Localhost);

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
            accounts.push_back(wallet);
        }

        Self {
            master_wallet,
            accounts,
            addresses,
        }
    }
}

fn pk_to_address(eth_pk: &H256) -> Address {
    PackedEthSignature::address_from_private_key(&eth_pk)
        .expect("Can't get an address from the private key")
}
