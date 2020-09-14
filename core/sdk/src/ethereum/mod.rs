//! Utilities for the on-chain operations, such as `Deposit` and `FullExit`.

use eth_client::{ETHClient, SignedCallResult};
use futures::compat::Compat01As03;
use models::{abi, node::TokenLike};
use web3::contract::tokens::Tokenize;
use web3::contract::{Contract, Options};
use web3::transports::{EventLoopHandle, Http};
use web3::types::{TransactionReceipt, H160, H256, U256};

use crate::{error::ClientError, provider::Provider, tokens_cache::TokensCache, types::Network};

const IERC20_INTERFACE: &str = include_str!("abi/IERC20.json");

const MAX_ERC20_APPROVE_AMOUNT: U256 =
    "115792089237316195423570985008687907853269984665640564039457584007913129639935"
        .parse()
        .unwrap(); // 2^256 - 1

const ERC20_APPROVE_TRESHOLD: U256 =
    "57896044618658097711785492504343953926634992332820282019728792003956564819968"
        .parse()
        .unwrap(); // 2^255

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
    provider: Provider,
    tokens_cache: TokensCache,
    erc20_abi: ethabi::Contract,
    // We have to prevent handle from drop, since it will cause event loop termination.
    _event_loop: EventLoopHandle,
}

impl EthereumProvider {
    pub async fn new(
        provider: Provider,
        tokens_cache: TokensCache,
        network: Network,
        eth_web3_url: impl AsRef<str>,
        eth_private_key: H256,
        eth_addr: H160,
    ) -> Result<Self, ClientError> {
        let transport = Http::new(eth_web3_url.as_ref())
            .map_err(|err| ClientError::NetworkError(err.to_string()))?;

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
            provider,
            eth_client,
            erc20_abi,
            tokens_cache,
            _event_loop,
        })
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
        let currentAllowance: U256 = query.await.map_err(|err| ClientError::NetworkError(err))?;

        Ok(false)
    }

    // pub fn send_deposit(&self, amount: ) -> Result<(), ClientError> {
    //     self.eth_client.encode_tx_data(
    //         "commitBlock",
    //         (
    //             u64::from(op.block.block_number),
    //             u64::from(op.block.fee_account),
    //             vec![root],
    //             public_data,
    //             witness_data.0,
    //             witness_data.1,
    //         ),
    //     );

    //     Ok(())
    // }
}
