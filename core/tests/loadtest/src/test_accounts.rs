// Built-in import
use std::sync::atomic::{AtomicU32, Ordering};
// External uses
use num::BigUint;
use rand::Rng;
// Workspace uses
use models::{
    helpers::closest_packable_fee_amount, tx::PackedEthSignature, AccountId, Address, FranklinTx,
    PriorityOp, TxFeeTypes,
};
use zksync::{
    error::ClientError, utils::biguint_to_u256, web3::types::H256, EthereumProvider, Network,
    Wallet, WalletCredentials,
};
use zksync_config::ConfigurationOptions;
// Local uses
use crate::{monitor::Monitor, scenarios::configs::AccountInfo};

#[derive(Debug)]
pub struct TestWallet {
    pub monitor: Monitor,
    pub eth_provider: EthereumProvider,
    inner: Wallet,
    nonce: AtomicU32,
}

impl TestWallet {
    pub const TOKEN_NAME: &'static str = "ETH";
    const FEE_FACTOR: u64 = 3;

    pub async fn from_info(
        monitor: Monitor,
        info: &AccountInfo,
        options: &ConfigurationOptions,
    ) -> Self {
        let credentials =
            WalletCredentials::from_eth_pk(info.address, info.private_key, Network::Localhost)
                .unwrap();

        let inner = Wallet::new(monitor.provider.clone(), credentials)
            .await
            .unwrap();
        Self::from_wallet(monitor, inner, &options.web3_url).await
    }

    // Parses and builds a new wallets list.
    pub async fn from_info_list(
        monitor: Monitor,
        input: &[AccountInfo],
        options: &ConfigurationOptions,
    ) -> Vec<Self> {
        let mut wallets = Vec::new();

        for info in input {
            let wallet = Self::from_info(monitor.clone(), info, options).await;
            wallets.push(wallet)
        }
        wallets
    }

    // Creates a random wallet.
    pub async fn new_random(monitor: Monitor, options: &ConfigurationOptions) -> Self {
        let eth_private_key = gen_random_eth_private_key();
        let address_from_pk =
            PackedEthSignature::address_from_private_key(&eth_private_key).unwrap();

        let inner = Wallet::new(
            monitor.provider.clone(),
            WalletCredentials::from_eth_pk(address_from_pk, eth_private_key, Network::Localhost)
                .unwrap(),
        )
        .await
        .unwrap();

        Self::from_wallet(monitor, inner, &options.web3_url).await
    }

    async fn from_wallet(monitor: Monitor, inner: Wallet, web3_url: impl AsRef<str>) -> Self {
        let eth_provider = inner.ethereum(web3_url).await.unwrap();
        let zk_nonce = inner
            .provider
            .account_info(inner.address())
            .await
            .unwrap()
            .committed
            .nonce;

        Self {
            monitor,
            inner,
            eth_provider,
            nonce: AtomicU32::new(zk_nonce),
        }
    }

    /// Returns the wallet address.
    pub fn address(&self) -> Address {
        self.inner.address()
    }

    /// Returns sufficient fee required to process each kind of transactions in zkSync network.
    pub async fn sufficient_fee(&self) -> Result<BigUint, ClientError> {
        let fee = self
            .monitor
            .provider
            .get_tx_fee(TxFeeTypes::Transfer, Address::zero(), Self::TOKEN_NAME)
            .await?
            .total_fee
            * BigUint::from(Self::FEE_FACTOR);

        Ok(closest_packable_fee_amount(&fee))
    }

    /// Returns the current account ID.
    pub fn account_id(&self) -> Option<AccountId> {
        self.inner.account_id()
    }

    // Updates ZKSync account id.
    pub async fn update_account_id(&mut self) -> Result<(), ClientError> {
        self.inner.update_account_id().await
    }

    // Creates a signed change public key transaction.
    pub async fn sign_change_pubkey(&self, fee: BigUint) -> Result<FranklinTx, ClientError> {
        self.inner
            .start_change_pubkey()
            .nonce(self.pending_nonce())
            .fee_token(Self::TOKEN_NAME)?
            .fee(fee)
            .tx()
            .await
    }

    // Creates a signed withdraw transaction.
    pub async fn sign_withdraw_single(
        &self,
        amount: BigUint,
    ) -> Result<(FranklinTx, Option<PackedEthSignature>), ClientError> {
        self.inner
            .start_withdraw()
            .nonce(self.pending_nonce())
            .token(Self::TOKEN_NAME)?
            .amount(amount)
            .to(self.inner.address())
            .tx()
            .await
    }

    // Creates a signed withdraw transaction with a fee provided.
    pub async fn sign_withdraw(
        &self,
        amount: BigUint,
        fee: Option<BigUint>,
    ) -> Result<(FranklinTx, Option<PackedEthSignature>), ClientError> {
        let mut builder = self
            .inner
            .start_withdraw()
            .nonce(self.pending_nonce())
            .token(Self::TOKEN_NAME)?
            .amount(amount)
            .to(self.inner.address());
        if let Some(fee) = fee {
            builder = builder.fee(fee);
        }

        builder.tx().await
    }

    // Creates a signed transfer tx to a given receiver.
    pub async fn sign_transfer(
        &self,
        to: impl Into<Address>,
        amount: impl Into<BigUint>,
        fee: Option<BigUint>,
    ) -> Result<(FranklinTx, Option<PackedEthSignature>), ClientError> {
        let mut builder = self
            .inner
            .start_transfer()
            .nonce(self.pending_nonce())
            .token(Self::TOKEN_NAME)?
            .amount(amount)
            .to(to.into());
        if let Some(fee) = fee {
            builder = builder.fee(fee);
        }

        builder.tx().await
    }

    // Creates a signed transfer tx to a random receiver.
    pub async fn sign_transfer_to_random(
        &self,
        test_accounts: &[AccountInfo],
        amount: BigUint,
    ) -> Result<(FranklinTx, Option<PackedEthSignature>), ClientError> {
        let to = {
            let mut rng = rand::thread_rng();
            let count = test_accounts.len() - 1;

            let mut to_idx = rng.gen_range(0, count);
            while test_accounts[to_idx].address == self.inner.address() {
                to_idx = rng.gen_range(0, count);
            }
            test_accounts[to_idx].address
        };

        self.sign_transfer(to, amount, None).await
    }

    // Deposits tokens from Ethereum to the contract.
    pub async fn deposit(&self, amount: impl Into<BigUint>) -> Result<PriorityOp, ClientError> {
        let eth_tx_hash = self
            .eth_provider
            .deposit(
                Self::TOKEN_NAME,
                biguint_to_u256(amount.into()),
                self.address(),
            )
            .await?;

        self.monitor
            .get_priority_op(&self.eth_provider, eth_tx_hash)
            .await
    }

    /// Returns appropriate nonce for the new transaction and increments the nonce.
    fn pending_nonce(&self) -> u32 {
        self.nonce.fetch_add(1, Ordering::SeqCst)
    }
}

fn gen_random_eth_private_key() -> H256 {
    let mut eth_private_key = H256::default();
    eth_private_key.randomize();
    eth_private_key
}
