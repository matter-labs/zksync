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
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::{
    tx::PackedEthSignature, AccountId, Address, PriorityOp, TokenLike, TxFeeTypes, ZkSyncTx,
};
// Local uses
use crate::{config::AccountInfo, monitor::Monitor, session::save_wallet};

/// A wrapper over `zksync::Wallet` to make testing more convenient.
#[derive(Debug)]
pub struct TestWallet {
    monitor: Monitor,
    eth_provider: EthereumProvider<PrivateKeySigner>,
    inner: Wallet<PrivateKeySigner>,
    token_name: TokenLike,

    nonce: AtomicU32,
}

impl TestWallet {
    const FEE_FACTOR: u64 = 3;

    /// Creates a new wallet from the given account information and Ethereum configuration options.
    pub async fn from_info(
        monitor: Monitor,
        info: &AccountInfo,
        options: &ConfigurationOptions,
    ) -> Self {
        let credentials = WalletCredentials::from_eth_signer(
            info.address,
            PrivateKeySigner::new(info.private_key),
            Network::Localhost,
        )
        .await
        .unwrap();

        let inner = Wallet::new(monitor.provider.clone(), credentials)
            .await
            .unwrap();

        let wallet =
            Self::from_wallet(info.token_name.clone(), monitor, inner, &options.web3_url).await;
        save_wallet(info.clone());
        wallet
    }

    /// Creates a random wallet.
    pub async fn new_random(
        token_name: TokenLike,
        monitor: Monitor,
        options: &ConfigurationOptions,
    ) -> Self {
        let eth_private_key = gen_random_eth_private_key();
        let address_from_pk =
            PackedEthSignature::address_from_private_key(&eth_private_key).unwrap();

        let info = AccountInfo {
            address: address_from_pk,
            private_key: eth_private_key,
            token_name,
        };

        Self::from_info(monitor, &info, options).await
    }

    async fn from_wallet(
        token_name: TokenLike,
        monitor: Monitor,
        inner: Wallet<PrivateKeySigner>,
        web3_url: impl AsRef<str>,
    ) -> Self {
        let eth_provider = inner.ethereum(web3_url).await.unwrap();
        let zk_nonce = inner
            .provider
            .account_info(inner.address())
            .await
            .unwrap()
            .committed
            .nonce;

        monitor
            .api_data_pool
            .write()
            .await
            .store_address(inner.address());

        Self {
            monitor,
            inner,
            eth_provider,
            nonce: AtomicU32::new(zk_nonce),
            token_name,
        }
    }

    /// Sets the correct nonce from the zkSync network.
    ///
    /// This method fixes further "nonce mismatch" errors.
    pub async fn refresh_nonce(&self) -> Result<(), ClientError> {
        let zk_nonce = self
            .inner
            .provider
            .account_info(self.address())
            .await?
            .committed
            .nonce;

        self.nonce.store(zk_nonce, Ordering::SeqCst);
        Ok(())
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

    /// Returns the token name of this wallet.
    pub fn token_name(&self) -> &TokenLike {
        &self.token_name
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

    // Performs a full exit operation.
    pub async fn full_exit(&self) -> anyhow::Result<PriorityOp> {
        let eth_tx_hash = self
            .eth_provider
            .full_exit(
                self.token_name.clone(),
                self.account_id()
                    .expect("An attempt to perform full exit on a wallet without account_id."),
            )
            .await?;

        self.monitor
            .get_priority_op(&self.eth_provider, eth_tx_hash)
            .await
    }

    /// Returns an underlying wallet.
    pub fn into_inner(self) -> Wallet<PrivateKeySigner> {
        self.inner
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
