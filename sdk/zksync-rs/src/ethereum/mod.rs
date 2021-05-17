//! Utilities for the on-chain operations, such as `Deposit` and `FullExit`.

use num::BigUint;
use serde_json::{Map, Value};
use std::{convert::TryFrom, time::Duration};
use std::{str::FromStr, time::Instant};
use web3::contract::tokens::Tokenize;
use web3::contract::Options;
use web3::transports::Http;
use web3::types::{TransactionReceipt, H160, H256, U256};

use zksync_eth_client::ETHDirectClient;
use zksync_eth_signer::EthereumSigner;
use zksync_types::{AccountId, Address, PriorityOp, PriorityOpId, TokenId, TokenLike};

use crate::{
    error::ClientError, provider::Provider, tokens_cache::TokensCache, utils::u256_to_biguint,
};

pub use self::priority_op_handle::PriorityOpHandle;
use zksync_crypto::params::MIN_NFT_TOKEN_ID;

mod priority_op_handle;

const IERC20_INTERFACE: &str = include_str!("abi/IERC20.json");
const ZKSYNC_INTERFACE: &str = include_str!("abi/ZkSync.json");
const RAW_ERC20_DEPOSIT_GAS_LIMIT: &str = include_str!("DepositERC20GasLimit.json");

fn load_contract(raw_abi_string: &str) -> ethabi::Contract {
    let abi_string = serde_json::Value::from_str(raw_abi_string)
        .expect("Malformed ZkSync contract file")
        .get("abi")
        .expect("Malformed ZkSync contract file")
        .to_string();
    ethabi::Contract::load(abi_string.as_bytes()).unwrap()
}

/// Returns `ethabi::Contract` object for zkSync smart contract.
pub fn zksync_contract() -> ethabi::Contract {
    load_contract(ZKSYNC_INTERFACE)
}

/// Returns `ethabi::Contract` object for ERC-20 smart contract interface.
pub fn ierc20_contract() -> ethabi::Contract {
    load_contract(IERC20_INTERFACE)
}

/// `EthereumProvider` gains access to on-chain operations, such as deposits and full exits.
/// Methods to interact with Ethereum return corresponding Ethereum transaction hash.
/// In order to monitor transaction execution, an Ethereum node `web3` API is exposed
/// via `EthereumProvider::web3` method.
#[derive(Debug)]
pub struct EthereumProvider<S: EthereumSigner> {
    tokens_cache: TokensCache,
    eth_client: ETHDirectClient<S>,
    erc20_abi: ethabi::Contract,
    confirmation_timeout: Duration,
}

impl<S: EthereumSigner> EthereumProvider<S> {
    /// Creates a new Ethereum provider.
    pub async fn new<P: Provider>(
        provider: &P,
        tokens_cache: TokensCache,
        eth_web3_url: impl AsRef<str>,
        eth_signer: S,
        eth_addr: H160,
    ) -> Result<Self, ClientError> {
        let transport = Http::new(eth_web3_url.as_ref())
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

        let network = provider.network();

        let address_response = provider.contract_address().await?;
        let contract_address =
            if let Some(main_contract) = address_response.main_contract.strip_prefix("0x") {
                main_contract
            } else {
                &address_response.main_contract
            };

        let eth_client = ETHDirectClient::new(
            transport,
            zksync_contract(),
            eth_addr,
            eth_signer,
            contract_address
                .parse()
                .map_err(|err| ClientError::MalformedResponse(format!("{}", err)))?,
            network.chain_id(),
            1.5f64,
        );
        let erc20_abi = ierc20_contract();

        Ok(Self {
            eth_client,
            erc20_abi,
            tokens_cache,
            confirmation_timeout: Duration::from_secs(10),
        })
    }

    /// Exposes Ethereum node `web3` API.
    pub fn client(&self) -> &ETHDirectClient<S> {
        &self.eth_client
    }

    /// Returns the zkSync contract address.
    pub fn contract_address(&self) -> H160 {
        self.client().contract_addr()
    }

    /// Returns the Ethereum account balance.
    pub async fn balance(&self) -> Result<BigUint, ClientError> {
        self.client()
            .sender_eth_balance()
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))
            .map(u256_to_biguint)
    }

    /// Returns the ERC20 token account balance.
    pub async fn erc20_balance(
        &self,
        address: Address,
        token: impl Into<TokenLike>,
    ) -> Result<U256, ClientError> {
        let token = self
            .tokens_cache
            .resolve(token.into())
            .ok_or(ClientError::UnknownToken)?;

        let res = self
            .eth_client
            .call_contract_function(
                "balanceOf",
                address,
                None,
                Options::default(),
                None,
                token.address,
                self.erc20_abi.clone(),
            )
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;
        Ok(res)
    }

    /// Returns the pending nonce for the Ethereum account.
    pub async fn nonce(&self) -> Result<U256, ClientError> {
        self.client()
            .pending_nonce()
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))
    }

    /// Checks whether ERC20 of a certain token deposit is approved for account.
    pub async fn is_erc20_deposit_approved(
        &self,
        token: impl Into<TokenLike>,
    ) -> Result<bool, ClientError> {
        self.is_limited_erc20_deposit_approved(token, U256::from(2).pow(255.into()))
            .await
    }

    /// Checks whether ERC20 of a certain token deposit with limit is approved for account.
    pub async fn is_limited_erc20_deposit_approved(
        &self,
        token: impl Into<TokenLike>,
        erc20_approve_threshold: U256,
    ) -> Result<bool, ClientError> {
        let token = token.into();
        let token = self
            .tokens_cache
            .resolve(token)
            .ok_or(ClientError::UnknownToken)?;

        let current_allowance = self
            .client()
            .allowance(token.address, self.erc20_abi.clone())
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

        Ok(current_allowance >= erc20_approve_threshold)
    }

    /// Sends a transaction to ERC20 token contract to approve the ERC20 deposit.
    pub async fn approve_erc20_token_deposits(
        &self,
        token: impl Into<TokenLike>,
    ) -> Result<H256, ClientError> {
        self.limited_approve_erc20_token_deposits(token, U256::max_value())
            .await
    }

    /// Sends a transaction to ERC20 token contract to approve the limited ERC20 deposit.
    pub async fn limited_approve_erc20_token_deposits(
        &self,
        token: impl Into<TokenLike>,
        max_erc20_approve_amount: U256,
    ) -> Result<H256, ClientError> {
        let token = token.into();

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
            .client()
            .sign_prepared_tx_for_addr(
                data,
                token.address,
                Options {
                    gas: Some(300_000.into()),
                    ..Default::default()
                },
            )
            .await
            .map_err(|_| ClientError::IncorrectCredentials)?;

        let transaction_hash = self
            .client()
            .send_raw_tx(signed_tx.raw_tx)
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

        Ok(transaction_hash)
    }

    /// Performs a transfer of funds from one Ethereum account to another.
    /// Note: This operation is performed on Ethereum, and not related to zkSync directly.
    pub async fn transfer(
        &self,
        token: impl Into<TokenLike>,
        amount: U256,
        to: H160,
    ) -> Result<H256, ClientError> {
        let token = token.into();
        let token_info = self
            .tokens_cache
            .resolve(token.clone())
            .ok_or(ClientError::UnknownToken)?;

        let signed_tx = if self.tokens_cache.is_eth(token) {
            let options = Options {
                value: Some(amount),
                gas: Some(300_000.into()),
                ..Default::default()
            };
            self.client()
                .sign_prepared_tx_for_addr(Vec::new(), to, options)
                .await
                .map_err(|_| ClientError::IncorrectCredentials)?
        } else {
            let contract_function = self
                .erc20_abi
                .function("transfer")
                .expect("failed to get function parameters");
            let params = (to, amount);
            let data = contract_function
                .encode_input(&params.into_tokens())
                .expect("failed to encode parameters");

            self.client()
                .sign_prepared_tx_for_addr(
                    data,
                    token_info.address,
                    Options {
                        gas: Some(300_000.into()),
                        ..Default::default()
                    },
                )
                .await
                .map_err(|_| ClientError::IncorrectCredentials)?
        };

        let transaction_hash = self
            .client()
            .send_raw_tx(signed_tx.raw_tx)
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

        Ok(transaction_hash)
    }

    #[cfg(feature = "mint")]
    pub async fn mint_erc20(
        &self,
        token: impl Into<TokenLike>,
        amount: U256,
        to: H160,
    ) -> Result<H256, ClientError> {
        let token = token.into();
        let token_info = self
            .tokens_cache
            .resolve(token.clone())
            .ok_or(ClientError::UnknownToken)?;

        if self.tokens_cache.is_eth(token) {
            // ETH minting is not supported
            return Err(ClientError::IncorrectInput);
        }

        let signed_tx = {
            let contract_function = self
                .erc20_abi
                .function("mint")
                .expect("failed to get function parameters");
            let params = (to, amount);
            let data = contract_function
                .encode_input(&params.into_tokens())
                .expect("failed to encode parameters");

            self.eth_client
                .sign_prepared_tx_for_addr(
                    data,
                    token_info.address,
                    Options {
                        gas: Some(300_000.into()),
                        ..Default::default()
                    },
                )
                .await
                .map_err(|_| ClientError::IncorrectCredentials)?
        };

        let transaction_hash = self
            .eth_client
            .send_raw_tx(signed_tx.raw_tx)
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

        Ok(transaction_hash)
    }

    /// Performs a deposit in zkSync network.
    /// For ERC20 tokens, a deposit must be approved beforehand via the `EthereumProvider::approve_erc20_token_deposits` method.
    pub async fn deposit(
        &self,
        token: impl Into<TokenLike>,
        amount: U256,
        sync_address: H160,
    ) -> Result<H256, ClientError> {
        let token = token.into();
        let token_info = self
            .tokens_cache
            .resolve(token.clone())
            .ok_or(ClientError::UnknownToken)?;

        let signed_tx = if self.tokens_cache.is_eth(token) {
            let options = Options {
                value: Some(amount),
                gas: Some(200_000.into()),
                ..Default::default()
            };
            let data = self.client().encode_tx_data("depositETH", sync_address);

            self.client()
                .sign_prepared_tx(data, options)
                .await
                .map_err(|_| ClientError::IncorrectCredentials)?
        } else {
            let gas_limits: Map<String, Value> = serde_json::from_str(RAW_ERC20_DEPOSIT_GAS_LIMIT)
                .map_err(|_| ClientError::Other)?;
            let address_str = format!("{:?}", token_info.address);
            let is_mainnet = self.client().chain_id() == 1;
            let gas_limit = if is_mainnet && gas_limits.contains_key(&address_str) {
                gas_limits
                    .get(&address_str)
                    .unwrap()
                    .as_u64()
                    .ok_or(ClientError::Other)?
            } else {
                300000u64
            };
            let options = Options {
                gas: Some(gas_limit.into()),
                ..Default::default()
            };
            let params = (token_info.address, amount, sync_address);
            let data = self.client().encode_tx_data("depositERC20", params);

            self.client()
                .sign_prepared_tx(data, options)
                .await
                .map_err(|_| ClientError::IncorrectCredentials)?
        };

        let transaction_hash = self
            .client()
            .send_raw_tx(signed_tx.raw_tx)
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

        Ok(transaction_hash)
    }

    /// Performs a full exit for a certain token.
    pub async fn full_exit(
        &self,
        token: impl Into<TokenLike>,
        account_id: AccountId,
    ) -> Result<H256, ClientError> {
        let token = token.into();
        let token = self
            .tokens_cache
            .resolve(token.clone())
            .ok_or(ClientError::UnknownToken)?;
        let account_id = U256::from(*account_id);

        let options = Options {
            gas: Some(500_000.into()),
            ..Default::default()
        };

        let data = self
            .client()
            .encode_tx_data("requestFullExit", (account_id, token.address));
        let signed_tx = self
            .client()
            .sign_prepared_tx(data, options)
            .await
            .map_err(|_| ClientError::IncorrectCredentials)?;

        let transaction_hash = self
            .client()
            .send_raw_tx(signed_tx.raw_tx)
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

        Ok(transaction_hash)
    }

    /// Performs a full exit for a certain nft.
    pub async fn full_exit_nft(
        &self,
        token: TokenId,
        account_id: AccountId,
    ) -> Result<H256, ClientError> {
        if token.0 < MIN_NFT_TOKEN_ID {
            return Err(ClientError::UnknownToken);
        }
        let account_id = U256::from(*account_id);
        let options = Options {
            gas: Some(500_000.into()),
            ..Default::default()
        };

        let data = self
            .eth_client
            .encode_tx_data("requestFullExitNFT", (account_id, token.0));
        let signed_tx = self
            .eth_client
            .sign_prepared_tx(data, options)
            .await
            .map_err(|_| ClientError::IncorrectCredentials)?;

        let transaction_hash = self
            .eth_client
            .send_raw_tx(signed_tx.raw_tx)
            .await
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

        Ok(transaction_hash)
    }

    /// Sets the timeout to wait for transactions to appear in the Ethereum network.
    /// By default it is set to 10 seconds.
    pub fn set_confirmation_timeout(&mut self, timeout: Duration) {
        self.confirmation_timeout = timeout;
    }

    /// Waits until the transaction is confirmed by the Ethereum blockchain.
    pub async fn wait_for_tx(&self, tx_hash: H256) -> Result<TransactionReceipt, ClientError> {
        let mut poller = tokio::time::interval(Duration::from_millis(100));

        let start = Instant::now();
        loop {
            if let Some(receipt) = self
                .client()
                .tx_receipt(tx_hash)
                .await
                .map_err(|err| ClientError::NetworkError(err.to_string()))?
            {
                return Ok(receipt);
            }

            if start.elapsed() > self.confirmation_timeout {
                return Err(ClientError::OperationTimeout);
            }
            poller.tick().await;
        }
    }
}

/// Trait describes the ability to receive the priority operation from this holder.
pub trait PriorityOpHolder {
    /// Returns the priority operation if exists.
    fn priority_op(&self) -> Option<PriorityOp>;

    /// Returns the handle for the priority operation.
    fn priority_op_handle<P: Provider>(&self, provider: P) -> Option<PriorityOpHandle<P>> {
        self.priority_op()
            .map(|op| PriorityOpHandle::new(PriorityOpId(op.serial_id), provider))
    }
}

impl PriorityOpHolder for TransactionReceipt {
    fn priority_op(&self) -> Option<PriorityOp> {
        self.logs
            .iter()
            .find_map(|op| PriorityOp::try_from(op.clone()).ok())
    }
}
