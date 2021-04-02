use web3::contract::tokens::{Detokenize, Tokenize};
use web3::contract::{Contract, Options};
use web3::transports::Http;
use web3::types::{Address, BlockId, Filter, Log, Transaction, U64};

use std::fmt::Debug;
use zksync_config::ZkSyncConfig;
use zksync_contracts::zksync_contract;
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::{TransactionReceipt, H160, H256, U256};

use crate::clients::mock::MockEthereum;
use crate::clients::multiplexer::MultiplexerEthereumClient;
use crate::ETHDirectClient;

#[derive(Debug, Clone, PartialEq)]
pub struct SignedCallResult {
    pub raw_tx: Vec<u8>,
    pub gas_price: U256,
    pub nonce: U256,
    pub hash: H256,
}

/// State of the executed Ethereum transaction.
#[derive(Debug, Clone)]
pub struct ExecutedTxStatus {
    /// Amount of confirmations for a block containing the transaction.
    pub confirmations: u64,
    /// Whether transaction was executed successfully or failed.
    pub success: bool,
    /// Receipt for a transaction. Will be set to `Some` only if the transaction
    /// failed during execution.
    pub receipt: Option<TransactionReceipt>,
}
/// Information about transaction failure.
#[derive(Debug, Clone)]
pub struct FailureInfo {
    pub revert_code: String,
    pub revert_reason: String,
    pub gas_used: Option<U256>,
    pub gas_limit: U256,
}

#[derive(Debug, Clone)]
pub enum EthereumGateway {
    Direct(ETHDirectClient<PrivateKeySigner>),
    Multiplexed(MultiplexerEthereumClient),
    Mock(MockEthereum),
}

impl EthereumGateway {
    pub fn from_config(config: &ZkSyncConfig) -> Self {
        if config.eth_client.web3_url.len() == 1 {
            let transport = web3::transports::Http::new(&config.eth_client.web3_url()).unwrap();

            EthereumGateway::Direct(ETHDirectClient::new(
                transport,
                zksync_contract(),
                config.eth_sender.sender.operator_commit_eth_addr,
                PrivateKeySigner::new(config.eth_sender.sender.operator_private_key),
                config.contracts.contract_addr,
                config.eth_client.chain_id,
                config.eth_client.gas_price_factor,
            ))
        } else {
            let mut client = MultiplexerEthereumClient::new();

            let contract = zksync_contract();
            for web3_url in config.eth_client.web3_url.iter() {
                let transport = web3::transports::Http::new(web3_url).unwrap();
                client.add_client(
                    web3_url.clone(),
                    ETHDirectClient::new(
                        transport,
                        contract.clone(),
                        config.eth_sender.sender.operator_commit_eth_addr,
                        PrivateKeySigner::new(config.eth_sender.sender.operator_private_key),
                        config.contracts.contract_addr,
                        config.eth_client.chain_id,
                        config.eth_client.gas_price_factor,
                    ),
                );
            }
            EthereumGateway::Multiplexed(client)
        }
    }
}

macro_rules! delegate_call {
    ($self:ident.$method:ident($($args:ident),*)) => {
        match $self {
            Self::Direct(d) => d.$method($($args),*).await,
            Self::Multiplexed(d) => d.$method($($args),*).await,
            Self::Mock(d) => d.$method($($args),*).await,
        }
    }
}

impl EthereumGateway {
    /// Returns the next *expected* nonce with respect to the transactions
    /// in the mempool.
    ///
    /// Note that this method may be inconsistent if used with a cluster of nodes
    /// (e.g. `infura`), since the consecutive tx send and attempt to get a pending
    /// nonce may be routed to the different nodes in cluster, and the latter node
    /// may not know about the send tx yet. Thus it is not recommended to rely on this
    /// method as on the trusted source of the latest nonce.
    pub async fn pending_nonce(&self) -> Result<U256, anyhow::Error> {
        delegate_call!(self.pending_nonce())
    }

    /// Returns the account nonce based on the last *mined* block. Not mined transactions
    /// (which are in mempool yet) are not taken into account by this method.
    pub async fn current_nonce(&self) -> Result<U256, anyhow::Error> {
        delegate_call!(self.current_nonce())
    }

    pub async fn block_number(&self) -> Result<U64, anyhow::Error> {
        delegate_call!(self.block_number())
    }

    pub async fn get_gas_price(&self) -> Result<U256, anyhow::Error> {
        delegate_call!(self.get_gas_price())
    }
    /// Returns the account balance.
    pub async fn sender_eth_balance(&self) -> Result<U256, anyhow::Error> {
        delegate_call!(self.sender_eth_balance())
    }

    /// Signs the transaction given the previously encoded data.
    /// Fills in gas/nonce if not supplied inside options.
    pub async fn sign_prepared_tx(
        &self,
        data: Vec<u8>,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error> {
        delegate_call!(self.sign_prepared_tx(data, options))
    }

    /// Signs the transaction given the previously encoded data.
    /// Fills in gas/nonce if not supplied inside options.
    pub async fn sign_prepared_tx_for_addr(
        &self,
        data: Vec<u8>,
        contract_addr: H160,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error> {
        delegate_call!(self.sign_prepared_tx_for_addr(data, contract_addr, options))
    }

    /// Sends the transaction to the Ethereum blockchain.
    /// Transaction is expected to be encoded as the byte sequence.
    pub async fn send_raw_tx(&self, tx: Vec<u8>) -> Result<H256, anyhow::Error> {
        delegate_call!(self.send_raw_tx(tx))
    }

    /// Gets the Ethereum transaction receipt.
    pub async fn tx_receipt(
        &self,
        tx_hash: H256,
    ) -> Result<Option<TransactionReceipt>, anyhow::Error> {
        delegate_call!(self.tx_receipt(tx_hash))
    }

    pub async fn failure_reason(
        &self,
        tx_hash: H256,
    ) -> Result<Option<FailureInfo>, anyhow::Error> {
        delegate_call!(self.failure_reason(tx_hash))
    }

    /// Auxiliary function that returns the balance of the account on Ethereum.
    pub async fn eth_balance(&self, address: Address) -> Result<U256, anyhow::Error> {
        delegate_call!(self.eth_balance(address))
    }

    pub async fn allowance(
        &self,
        token_address: Address,
        erc20_abi: ethabi::Contract,
    ) -> Result<U256, anyhow::Error> {
        delegate_call!(self.allowance(token_address, erc20_abi))
    }

    pub async fn get_tx_status(&self, hash: H256) -> anyhow::Result<Option<ExecutedTxStatus>> {
        delegate_call!(self.get_tx_status(hash))
    }

    /// Encodes the transaction data (smart contract method and its input) to the bytes
    /// without creating an actual transaction.
    pub async fn call_main_contract_function<R, A, B, P>(
        &self,
        func: &str,
        params: P,
        from: A,
        options: Options,
        block: B,
    ) -> Result<R, anyhow::Error>
    where
        R: Detokenize + Unpin,
        A: Into<Option<Address>> + Clone,
        B: Into<Option<BlockId>> + Clone,
        P: Tokenize + Clone,
    {
        delegate_call!(self.call_main_contract_function(func, params, from, options, block))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn call_contract_function<R, A, B, P>(
        &self,
        func: &str,
        params: P,
        from: A,
        options: Options,
        block: B,
        token_address: Address,
        erc20_abi: ethabi::Contract,
    ) -> Result<R, anyhow::Error>
    where
        R: Detokenize + Unpin,
        A: Into<Option<Address>> + Clone,
        B: Into<Option<BlockId>> + Clone,
        P: Tokenize + Clone,
    {
        delegate_call!(self.call_contract_function(
            func,
            params,
            from,
            options,
            block,
            token_address,
            erc20_abi
        ))
    }

    pub async fn logs(&self, filter: Filter) -> anyhow::Result<Vec<Log>> {
        delegate_call!(self.logs(filter))
    }

    pub fn encode_tx_data<P: Tokenize + Clone>(&self, func: &str, params: P) -> Vec<u8> {
        match self {
            EthereumGateway::Multiplexed(c) => c.encode_tx_data(func, params),
            EthereumGateway::Direct(c) => c.encode_tx_data(func, params),
            EthereumGateway::Mock(c) => c.encode_tx_data(func, params),
        }
    }

    pub fn create_contract(&self, address: Address, contract: ethabi::Contract) -> Contract<Http> {
        match self {
            EthereumGateway::Multiplexed(c) => c.create_contract(address, contract),
            EthereumGateway::Direct(c) => c.create_contract(address, contract),
            EthereumGateway::Mock(c) => c.create_contract(address, contract),
        }
    }

    pub async fn get_tx(&self, hash: H256) -> anyhow::Result<Option<Transaction>> {
        delegate_call!(self.get_tx(hash))
    }

    pub fn is_multiplexed(&self) -> bool {
        matches!(self, EthereumGateway::Multiplexed(_))
    }

    pub fn get_mut_mock(&mut self) -> Option<&mut MockEthereum> {
        match self {
            EthereumGateway::Mock(ref mut m) => Some(m),
            _ => None,
        }
    }

    pub fn get_mock(&self) -> Option<&MockEthereum> {
        match self {
            EthereumGateway::Mock(m) => Some(&m),
            _ => None,
        }
    }
}
