// Built-in import
use std::sync::atomic::{AtomicU32, Ordering};
// External uses
use num::BigUint;
// Workspace uses
use zksync::{
    error::ClientError,
    types::BlockStatus,
    utils::{biguint_to_u256, closest_packable_fee_amount},
    web3::types::H256,
    EthereumProvider, Network, Wallet, WalletCredentials,
};
use zksync_config::ConfigurationOptions;
use zksync_types::{
    tx::PackedEthSignature, AccountId, Address, PriorityOp, TokenLike, TxFeeTypes, ZkSyncTx,
};
// Local uses
use crate::{config::AccountInfo, monitor::Monitor};

#[derive(Debug)]
pub struct TestWallet {
    monitor: Monitor,
    eth_provider: EthereumProvider,
    inner: Wallet,
    token_name: TokenLike,

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
            token_name: Self::TOKEN_NAME.into(),
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
            .get_tx_fee(
                TxFeeTypes::FastWithdraw,
                Address::zero(),
                self.token_name.clone(),
            )
            .await?
            .total_fee
            * BigUint::from(Self::FEE_FACTOR);

        Ok(closest_packable_fee_amount(&fee))
    }

    /// Returns the wallet balance in zkSync network.
    pub async fn balance(&self, block_status: BlockStatus) -> Result<BigUint, ClientError> {
        self.inner
            .get_balance(block_status, self.token_name.clone())
            .await
    }

    /// Returns the wallet balance in Ehtereum network.
    pub async fn eth_balance(&self) -> Result<BigUint, ClientError> {
        self.eth_provider.balance().await
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
    pub async fn sign_change_pubkey(
        &self,
        fee: impl Into<BigUint>,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        let tx = self
            .inner
            .start_change_pubkey()
            .nonce(self.pending_nonce())
            .fee_token(self.token_name.clone())?
            .fee(fee)
            .tx()
            .await?;

        Ok((tx, None))
    }

    // Creates a signed withdraw transaction with a fee provided.
    pub async fn sign_withdraw(
        &self,
        amount: impl Into<BigUint>,
        fee: impl Into<BigUint>,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        self.inner
            .start_withdraw()
            .nonce(self.pending_nonce())
            .token(self.token_name.clone())?
            .amount(amount)
            .fee(fee)
            .to(self.inner.address())
            .tx()
            .await
    }

    // Creates a signed transfer tx to a given receiver.
    pub async fn sign_transfer(
        &self,
        to: impl Into<Address>,
        amount: impl Into<BigUint>,
        fee: BigUint,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        self.inner
            .start_transfer()
            .nonce(self.pending_nonce())
            .token(self.token_name.clone())?
            .amount(amount)
            .fee(fee)
            .to(to.into())
            .tx()
            .await
    }

    // Deposits tokens from Ethereum to the contract.
    pub async fn deposit(&self, amount: impl Into<BigUint>) -> anyhow::Result<PriorityOp> {
        let eth_tx_hash = self
            .eth_provider
            .deposit(
                self.token_name.clone(),
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
