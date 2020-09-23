// Built-in import
// External uses
use num::BigUint;
use rand::Rng;
// Workspace uses
use models::{
    config_options::ConfigurationOptions,
    node::{tx::PackedEthSignature, FranklinTx},
};
use zksync::{
    error::ClientError, web3::types::H256, EthereumProvider, Provider, Wallet, WalletCredentials,
};
// Local uses
use crate::scenarios::configs::AccountInfo;

#[derive(Debug)]
pub struct TestWallet {
    pub zk_wallet: Wallet,
    pub eth_provider: EthereumProvider,
}

impl TestWallet {
    pub const TOKEN_NAME: &'static str = "ETH";

    pub async fn from_info(
        info: &AccountInfo,
        provider: Provider,
        config: &ConfigurationOptions,
    ) -> Self {
        let credentials = WalletCredentials::from_eth_pk(info.address, info.private_key).unwrap();

        let zk_wallet = Wallet::new(provider, credentials).await.unwrap();
        let eth_provider = zk_wallet.ethereum(&config.web3_url).await.unwrap();

        Self {
            zk_wallet,
            eth_provider,
        }
    }

    // Parses and builds a new wallets list.
    pub async fn from_info_list(
        input: &[AccountInfo],
        provider: Provider,
        config: &ConfigurationOptions,
    ) -> Vec<Self> {
        let mut wallets = Vec::new();

        for info in input {
            let wallet = Self::from_info(info, provider.clone(), config).await;
            wallets.push(wallet)
        }
        wallets
    }

    // Updates ZKSync account id.
    pub async fn update_account_id(&mut self) -> Result<(), ClientError> {
        self.zk_wallet.update_account_id().await
    }

    // Creates a signed change public key transaction.
    pub async fn sign_change_pubkey(&self) -> Result<FranklinTx, ClientError> {
        self.zk_wallet.start_change_pubkey().tx().await
    }

    // Creates a signed withdraw transaction.
    pub async fn sign_withdraw_single(
        &self,
        amount: BigUint,
    ) -> Result<(FranklinTx, Option<PackedEthSignature>), ClientError> {
        self.zk_wallet
            .start_withdraw()
            .token(Self::TOKEN_NAME)?
            .amount(amount)
            .fee(0u32)
            .to(self.zk_wallet.address())
            .tx()
            .await
    }

    // Creates a signed withdraw transaction with a fee provided.
    pub async fn sign_withdraw(
        &self,
        amount: BigUint,
        fee: BigUint,
    ) -> Result<(FranklinTx, Option<PackedEthSignature>), ClientError> {
        self.zk_wallet
            .start_withdraw()
            .token(Self::TOKEN_NAME)?
            .amount(amount)
            .fee(fee)
            .to(self.zk_wallet.address())
            .tx()
            .await
    }

    // Creates a signed transfer tx to a random receiver.
    pub async fn sign_transfer_to_random(
        &self,
        test_accounts: &[AccountInfo],
        amount: BigUint,
    ) -> Result<(FranklinTx, Option<PackedEthSignature>), ClientError> {
        let to = {
            let mut to_idx = rand::thread_rng().gen_range(0, test_accounts.len() - 1);
            while test_accounts[to_idx].address == self.zk_wallet.address() {
                to_idx = rand::thread_rng().gen_range(0, test_accounts.len() - 1);
            }
            test_accounts[to_idx].address
        };

        self.zk_wallet
            .start_transfer()
            .token(Self::TOKEN_NAME)?
            .amount(amount)
            .fee(0u32)
            .to(to)
            .tx()
            .await
    }
}

pub(crate) fn gen_random_eth_private_key() -> H256 {
    let mut eth_private_key = H256::default();
    eth_private_key.randomize();
    eth_private_key
}

pub(crate) async fn gen_random_wallet(provider: Provider) -> Wallet {
    let eth_private_key = gen_random_eth_private_key();
    let address_from_pk = PackedEthSignature::address_from_private_key(&eth_private_key).unwrap();

    Wallet::new(
        provider,
        WalletCredentials::from_eth_pk(address_from_pk, eth_private_key).unwrap(),
    )
    .await
    .unwrap()
}
