// Built-in import
use std::sync::atomic::{AtomicU32, Ordering};
// External uses
use num::BigUint;
// Workspace uses
use zksync::{
    error::ClientError,
    ethereum::ierc20_contract,
    provider::Provider,
    types::BlockStatus,
    utils::{biguint_to_u256, closest_packable_fee_amount, u256_to_biguint},
    web3::{contract::Options, types::H256},
    EthereumProvider, Network, RpcProvider, Wallet,
};
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::{
    tx::PackedEthSignature, AccountId, Address, Nonce, PriorityOp, TokenLike, TxFeeTypes, ZkSyncTx,
};
// Local uses
use crate::{config::WalletCredentials, monitor::Monitor, session::save_wallet};

/// A main loadtest wallet with the enough amount of gas and tokens to perform scenarios.
#[derive(Debug)]
pub struct MainWallet {
    monitor: Monitor,
    eth_provider: EthereumProvider<PrivateKeySigner>,
    inner: Wallet<PrivateKeySigner, RpcProvider>,

    nonce: AtomicU32,
}

impl MainWallet {
    const FEE_FACTOR: u64 = 3;

    /// Creates a new wallet from the given account information and Ethereum configuration options.
    pub async fn new(
        monitor: Monitor,
        network: Network,
        credentials: WalletCredentials,
        web3_url: &str,
    ) -> Self {
        let zksync_credentials = zksync::WalletCredentials::from_eth_signer(
            credentials.address,
            PrivateKeySigner::new(credentials.private_key),
            network,
        )
        .await
        .unwrap();

        let inner = Wallet::new(monitor.provider.clone(), zksync_credentials)
            .await
            .unwrap();

        let wallet = Self::from_wallet(monitor, inner, web3_url).await;
        save_wallet(credentials);
        wallet
    }

    async fn from_wallet(
        monitor: Monitor,
        inner: Wallet<PrivateKeySigner, RpcProvider>,
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
            nonce: AtomicU32::new(*zk_nonce),
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

        self.nonce.store(*zk_nonce, Ordering::SeqCst);
        Ok(())
    }

    /// Returns appropriate nonce for the new transaction and increments the nonce.
    fn pending_nonce(&self) -> Nonce {
        Nonce(self.nonce.fetch_add(1, Ordering::SeqCst))
    }

    /// Returns the wallet address.
    pub fn address(&self) -> Address {
        self.inner.address()
    }

    /// Returns the current account ID.
    pub fn account_id(&self) -> Option<AccountId> {
        self.inner.account_id()
    }

    // Updates ZKSync account id.
    pub async fn update_account_id(&mut self) -> Result<(), ClientError> {
        self.inner.update_account_id().await?;
        if let Some(account_id) = self.account_id() {
            self.monitor
                .api_data_pool
                .write()
                .await
                .set_account_id(self.address(), account_id);
        }
        Ok(())
    }

    /// Returns sufficient fee required to process each kind of transactions in zkSync network.
    pub async fn sufficient_fee(
        &self,
        token_name: impl Into<TokenLike>,
    ) -> Result<BigUint, ClientError> {
        let fee = self
            .monitor
            .provider
            .get_tx_fee(TxFeeTypes::FastWithdraw, Address::zero(), token_name.into())
            .await?
            .total_fee
            * BigUint::from(Self::FEE_FACTOR);

        Ok(closest_packable_fee_amount(&fee))
    }

    /// Returns the wallet gas balance.
    pub async fn eth_balance(&self) -> Result<BigUint, ClientError> {
        self.eth_provider.balance().await
    }

    /// Returns the wallet balance in zkSync network of the specified token.
    pub async fn balance(
        &self,
        token_name: impl Into<TokenLike>,
        block_status: BlockStatus,
    ) -> Result<BigUint, ClientError> {
        self.inner.get_balance(block_status, token_name).await
    }

    /// Returns erc20 token balance in Ethereum network.
    pub async fn erc20_balance(
        &self,
        token_name: impl Into<TokenLike>,
    ) -> Result<BigUint, ClientError> {
        let token = self
            .inner
            .tokens
            .resolve(token_name.into())
            .ok_or(ClientError::UnknownToken)?;

        let contract = self
            .eth_provider
            .client()
            .create_contract(token.address, ierc20_contract());

        let balance = contract
            .query("balanceOf", self.address(), None, Options::default(), None)
            .await
            .map(u256_to_biguint)
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

        Ok(balance)
    }

    /// Returns eth balance if the given token is ETH; otherwise returns erc20 balance.
    pub async fn l1_balance(&self, token_name: &TokenLike) -> Result<BigUint, ClientError> {
        if token_name.is_eth() {
            self.eth_balance().await
        } else {
            self.erc20_balance(token_name).await
        }
    }

    /// Creates a signed change public key transaction.
    pub async fn sign_change_pubkey(
        &self,
        token_name: impl Into<TokenLike>,
        fee: impl Into<BigUint>,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        let tx = self
            .inner
            .start_change_pubkey()
            .nonce(self.pending_nonce())
            .fee_token(token_name)?
            .fee(fee)
            .tx()
            .await?;

        Ok((tx, None))
    }

    /// Creates a signed withdraw transaction with a fee provided for the given token.
    pub async fn sign_withdraw(
        &self,
        token_name: impl Into<TokenLike>,
        amount: impl Into<BigUint>,
        fee: impl Into<BigUint>,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        self.inner
            .start_withdraw()
            .nonce(self.pending_nonce())
            .token(token_name)?
            .amount(amount)
            .fee(fee)
            .to(self.inner.address())
            .tx()
            .await
    }

    /// Creates a signed transfer tx to a given receiver.
    pub async fn sign_transfer(
        &self,
        token_name: impl Into<TokenLike>,
        to: impl Into<Address>,
        amount: impl Into<BigUint>,
        fee: BigUint,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        self.inner
            .start_transfer()
            .nonce(self.pending_nonce())
            .token(token_name)?
            .amount(amount)
            .fee(fee)
            .to(to.into())
            .tx()
            .await
    }

    /// Deposits given token from the Ethereum network to the zkSync.
    pub async fn deposit(
        &self,
        token_name: impl Into<TokenLike>,
        amount: impl Into<BigUint>,
    ) -> anyhow::Result<PriorityOp> {
        let eth_tx_hash = self
            .eth_provider
            .deposit(token_name, biguint_to_u256(amount.into()), self.address())
            .await?;

        self.monitor
            .get_priority_op(&self.eth_provider, eth_tx_hash)
            .await
    }

    /// Performs a full exit operation for the specified token.
    pub async fn full_exit(&self, token_name: impl Into<TokenLike>) -> anyhow::Result<PriorityOp> {
        let eth_tx_hash = self
            .eth_provider
            .full_exit(
                token_name,
                self.account_id()
                    .expect("An attempt to perform full exit on a wallet without account_id."),
            )
            .await?;

        self.monitor
            .get_priority_op(&self.eth_provider, eth_tx_hash)
            .await
    }

    /// Sends a transaction to ERC20 token contract to approve the ERC20 deposit.
    pub async fn approve_erc20_deposits(
        &self,
        token_name: impl Into<TokenLike>,
    ) -> anyhow::Result<()> {
        let tx_hash = self
            .eth_provider
            .approve_erc20_token_deposits(token_name)
            .await?;
        self.eth_provider.wait_for_tx(tx_hash).await?;

        Ok(())
    }

    /// Sends a some amount tokens to the given address in the Ethereum network.
    pub async fn transfer_to(
        &self,
        token_name: impl Into<TokenLike>,
        amount: impl Into<BigUint>,
        to: Address,
    ) -> anyhow::Result<()> {
        let tx_hash = self
            .eth_provider
            .transfer(token_name, biguint_to_u256(amount.into()), to)
            .await?;
        self.eth_provider.wait_for_tx(tx_hash).await?;

        Ok(())
    }
}

/// A wallet used in a specific loadtest scenario.
#[derive(Debug)]
pub struct ScenarioWallet {
    inner: MainWallet,
    token_name: TokenLike,
}

impl ScenarioWallet {
    /// Creates a random scenario wallet.
    pub async fn new_random(
        monitor: Monitor,
        network: Network,
        token_name: TokenLike,
        web3_url: &str,
    ) -> Self {
        let eth_private_key = gen_random_eth_private_key();
        let address_from_pk =
            PackedEthSignature::address_from_private_key(&eth_private_key).unwrap();

        let credentials = WalletCredentials {
            address: address_from_pk,
            private_key: eth_private_key,
        };

        Self {
            inner: MainWallet::new(monitor, network, credentials, web3_url).await,
            token_name,
        }
    }

    /// Returns an underlying wallet.
    pub fn into_inner(self) -> Wallet<PrivateKeySigner, RpcProvider> {
        self.inner.inner
    }

    /// Sets the correct nonce from the zkSync network.
    ///
    /// This method fixes further "nonce mismatch" errors.
    pub async fn refresh_nonce(&self) -> Result<(), ClientError> {
        self.inner.refresh_nonce().await
    }

    /// Returns the wallet address.
    pub fn address(&self) -> Address {
        self.inner.address()
    }

    /// Returns the current account ID.
    pub fn account_id(&self) -> Option<AccountId> {
        self.inner.account_id()
    }

    /// Returns the token name of this wallet.
    pub fn token_name(&self) -> &TokenLike {
        &self.token_name
    }

    // Updates ZKSync account id.
    pub async fn update_account_id(&mut self) -> Result<(), ClientError> {
        self.inner.update_account_id().await
    }

    /// Returns sufficient fee required to process each kind of transactions in zkSync network.
    pub async fn sufficient_fee(&self) -> Result<BigUint, ClientError> {
        self.inner.sufficient_fee(&self.token_name).await
    }

    /// Returns the wallet gas balance.
    pub async fn eth_balance(&self) -> Result<BigUint, ClientError> {
        self.inner.eth_balance().await
    }

    /// Returns the wallet balance in zkSync network of the specified token.
    pub async fn balance(&self, block_status: BlockStatus) -> Result<BigUint, ClientError> {
        self.inner.balance(&self.token_name, block_status).await
    }

    /// Returns erc20 wallet balance in Ethereum network.
    pub async fn erc20_balance(&self) -> Result<BigUint, ClientError> {
        self.inner.erc20_balance(&self.token_name).await
    }

    /// Returns eth balance if the wallet is ETH; otherwise returns erc20 balance.
    pub async fn l1_balance(&self) -> Result<BigUint, ClientError> {
        self.inner.l1_balance(&self.token_name).await
    }

    /// Creates a signed change public key transaction.
    pub async fn sign_change_pubkey(
        &self,
        fee: impl Into<BigUint>,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        self.inner.sign_change_pubkey(&self.token_name, fee).await
    }

    /// Creates a signed withdraw transaction with a fee provided.
    pub async fn sign_withdraw(
        &self,
        amount: impl Into<BigUint>,
        fee: impl Into<BigUint>,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        self.inner
            .sign_withdraw(&self.token_name, amount, fee)
            .await
    }

    /// Creates a signed transfer tx to a given receiver.
    pub async fn sign_transfer(
        &self,
        to: impl Into<Address>,
        amount: impl Into<BigUint>,
        fee: BigUint,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        self.inner
            .sign_transfer(&self.token_name, to, amount, fee)
            .await
    }

    /// Deposits given token from the Ethereum network to the zkSync.
    pub async fn deposit(&self, amount: impl Into<BigUint>) -> anyhow::Result<PriorityOp> {
        self.inner.deposit(&self.token_name, amount).await
    }

    /// Performs a full exit operation.
    pub async fn full_exit(&self) -> anyhow::Result<PriorityOp> {
        self.inner.full_exit(&self.token_name).await
    }

    /// Sends a transaction to ERC20 token contract to approve the ERC20 deposit.
    pub async fn approve_erc20_deposits(&self) -> anyhow::Result<()> {
        self.inner.approve_erc20_deposits(&self.token_name).await
    }

    /// Sends a some amount tokens to the given address in the Ethereum network.
    pub async fn transfer_to(
        &self,
        token_name: impl Into<TokenLike>,
        amount: impl Into<BigUint>,
        to: Address,
    ) -> anyhow::Result<()> {
        self.inner.transfer_to(token_name, amount, to).await
    }
}

fn gen_random_eth_private_key() -> H256 {
    let mut eth_private_key = H256::default();
    eth_private_key.randomize();
    eth_private_key
}
