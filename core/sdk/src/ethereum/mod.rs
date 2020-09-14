//! Utilities for the on-chain operations, such as `Deposit` and `FullExit`.

use eth_client::ETHClient;
use futures::compat::Future01CompatExt;
use models::{
    abi,
    node::{AccountId, TokenLike},
};
use web3::contract::tokens::Tokenize;
use web3::contract::{Contract, Options};
use web3::transports::{EventLoopHandle, Http};
use web3::types::{H160, H256, U256};
use web3::Web3;

use crate::{error::ClientError, provider::Provider, tokens_cache::TokensCache, types::Network};

const IERC20_INTERFACE: &str = include_str!("abi/IERC20.json");

fn chain_id(network: Network) -> u8 {
    match network {
        Network::Mainnet => 1,
        Network::Ropsten => 3,
        Network::Rinkeby => 4,
        Network::Localhost => 9,
        Network::Unknown => panic!("Attempt to connect to an unknown network"),
    }
}

pub struct EthereumProvider {
    tokens_cache: TokensCache,
    eth_client: ETHClient<Http>,
    erc20_abi: ethabi::Contract,
    // We have to prevent handle from drop, since it will cause event loop termination.
    _event_loop: EventLoopHandle,
}

impl EthereumProvider {
    pub async fn new(
        provider: &Provider,
        tokens_cache: TokensCache,
        eth_web3_url: impl AsRef<str>,
        eth_private_key: H256,
        eth_addr: H160,
    ) -> Result<Self, ClientError> {
        let (_event_loop, transport) = Http::new(eth_web3_url.as_ref())
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

        let network = provider.network;

        let contract_address = provider.contract_address().await?;

        let eth_client = ETHClient::new(
            transport,
            abi::zksync_contract(),
            eth_addr,
            eth_private_key,
            contract_address
                .main_contract
                .parse()
                .map_err(|_| ClientError::MalformedResponse)?,
            chain_id(network),
            1.5f64,
        );

        let erc20_abi = ethabi::Contract::load(IERC20_INTERFACE.as_bytes()).unwrap();

        Ok(Self {
            eth_client,
            erc20_abi,
            tokens_cache,
            _event_loop,
        })
    }

    pub fn web3(&self) -> &Web3<Http> {
        &self.eth_client.web3
    }

    pub fn contract_address(&self) -> H160 {
        self.eth_client.contract_addr
    }

    pub async fn nonce(&self) -> Result<U256, ClientError> {
        self.eth_client
            .pending_nonce()
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))
    }

    pub async fn is_erc20_deposit_approved(&self, token: TokenLike) -> Result<bool, ClientError> {
        let erc20_approve_threshold: U256 =
            "57896044618658097711785492504343953926634992332820282019728792003956564819968"
                .parse()
                .unwrap(); // 2^255

        let token = self
            .tokens_cache
            .resolve(token)
            .ok_or(ClientError::UnknownToken)?;

        let contract = Contract::new(
            self.eth_client.web3.eth(),
            token.address,
            self.erc20_abi.clone(),
        );

        let query = contract.query(
            "allowance",
            (self.eth_client.sender_account, self.contract_address()),
            None,
            Options::default(),
            None,
        );
        let current_allowance: U256 = query
            .compat()
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

        Ok(current_allowance >= erc20_approve_threshold)
    }

    pub async fn approve_erc20_token_deposits(
        &self,
        token: TokenLike,
    ) -> Result<H256, ClientError> {
        let max_erc20_approve_amount: U256 =
            "115792089237316195423570985008687907853269984665640564039457584007913129639935"
                .parse()
                .unwrap(); // 2^256 - 1

        let token = self
            .tokens_cache
            .resolve(token)
            .ok_or(ClientError::UnknownToken)?;

        let contract_function = self
            .erc20_abi
            .function("approve")
            .expect("failed to get function parameters");
        let params = (self.contract_address(), max_erc20_approve_amount);
        let data = contract_function
            .encode_input(&params.into_tokens())
            .expect("failed to encode parameters");

        let signed_tx = self
            .eth_client
            .sign_prepared_tx_for_contract(data, token.address, Default::default())
            .await
            .map_err(|_| ClientError::IncorrectCredentials)?;

        let transactin_hash = self
            .eth_client
            .send_raw_tx(signed_tx.raw_tx)
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

        Ok(transactin_hash)
    }

    pub async fn deposit_erc20(
        &self,
        token: TokenLike,
        amount: U256,
        sync_address: H160,
    ) -> Result<H256, ClientError> {
        let token_info = self
            .tokens_cache
            .resolve(token.clone())
            .ok_or(ClientError::UnknownToken)?;

        let transactin_hash = if self.tokens_cache.is_eth(token) {
            let mut options = Options::default();
            options.value = Some(amount);
            options.gas = Some(200_000.into());
            let signed_tx = self
                .eth_client
                .sign_call_tx("depositETH", sync_address, options)
                .await
                .map_err(|_| ClientError::IncorrectCredentials)?;

            self.eth_client
                .send_raw_tx(signed_tx.raw_tx)
                .await
                .map_err(|err| ClientError::NetworkError(err.to_string()))?
        } else {
            let mut options = Options::default();
            options.gas = Some(200_000.into());
            let params = (token_info.address, amount, sync_address);
            let signed_tx = self
                .eth_client
                .sign_call_tx("depositETH", params, options)
                .await
                .map_err(|_| ClientError::IncorrectCredentials)?;

            self.eth_client
                .send_raw_tx(signed_tx.raw_tx)
                .await
                .map_err(|err| ClientError::NetworkError(err.to_string()))?
        };

        Ok(transactin_hash)
    }

    pub async fn full_exit(
        &self,
        token: TokenLike,
        account_id: AccountId,
    ) -> Result<H256, ClientError> {
        let token = self
            .tokens_cache
            .resolve(token.clone())
            .ok_or(ClientError::UnknownToken)?;
        let account_id = U256::from(account_id);

        let mut options = Options::default();
        options.gas = Some(500_000.into());

        let signed_tx = self
            .eth_client
            .sign_call_tx("fullExit", (account_id, token.address), options)
            .await
            .map_err(|_| ClientError::IncorrectCredentials)?;

        let transactin_hash = self
            .eth_client
            .send_raw_tx(signed_tx.raw_tx)
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

        Ok(transactin_hash)
    }
}
