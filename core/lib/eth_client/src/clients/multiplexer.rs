use crate::eth_client_trait::{ExecutedTxStatus, FailureInfo, SignedCallResult};
use ethabi::Contract;
use web3::contract::Options;
use web3::types::{Address, U64};

use crate::ETHClient;

use zksync_eth_signer::PrivateKeySigner;
use zksync_types::{TransactionReceipt, H160, H256, U256};

#[derive(Debug, Clone)]
pub struct MultiPlexClient {
    clients: Vec<(String, ETHClient<PrivateKeySigner>)>,
    contract: ethabi::Contract,
}

impl MultiPlexClient {
    pub fn new(contract: ethabi::Contract) -> Self {
        Self {
            clients: vec![],
            contract,
        }
    }

    pub fn add_client(mut self, name: String, client: ETHClient<PrivateKeySigner>) -> Self {
        self.clients.push((name, client));
        self
    }

    pub async fn pending_nonce(&self) -> Result<U256, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.pending_nonce().await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    pub async fn current_nonce(&self) -> Result<U256, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.current_nonce().await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    pub async fn block_number(&self) -> Result<U64, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.block_number().await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    pub async fn get_gas_price(&self) -> Result<U256, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.get_gas_price().await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    pub async fn balance(&self) -> Result<U256, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.balance().await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    pub async fn sign_prepared_tx(
        &self,
        data: Vec<u8>,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.sign_prepared_tx(data.clone(), options.clone()).await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    pub async fn sign_prepared_tx_for_addr(
        &self,
        data: Vec<u8>,
        contract_addr: H160,
        options: Options,
    ) -> Result<SignedCallResult, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client
                .sign_prepared_tx_for_addr(data.clone(), contract_addr, options.clone())
                .await
            {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    pub async fn send_raw_tx(&self, tx: Vec<u8>) -> Result<H256, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.send_raw_tx(tx.clone()).await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    pub async fn tx_receipt(
        &self,
        tx_hash: H256,
    ) -> Result<Option<TransactionReceipt>, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.tx_receipt(tx_hash).await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    pub async fn failure_reason(
        &self,
        tx_hash: H256,
    ) -> Result<Option<FailureInfo>, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.failure_reason(tx_hash).await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    pub async fn eth_balance(&self, address: Address) -> Result<U256, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.eth_balance(address).await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    pub async fn contract_balance(
        &self,
        token_address: Address,
        abi: ethabi::Contract,
        address: Address,
    ) -> Result<U256, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client
                .contract_balance(token_address, abi.clone(), address)
                .await
            {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    pub async fn allowance(
        &self,
        token_address: Address,
        erc20_abi: Contract,
    ) -> Result<U256, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.allowance(token_address, erc20_abi.clone()).await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    pub async fn get_tx_status(
        &self,
        hash: &H256,
    ) -> Result<Option<ExecutedTxStatus>, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.get_tx_status(hash).await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }
    pub fn contract(&self) -> &Contract {
        &self.contract
    }
}
