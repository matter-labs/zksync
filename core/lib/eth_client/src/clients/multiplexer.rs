use crate::eth_client_trait::{ETHClientSender, ETHTxEncoder, FailureInfo, SignedCallResult};
use ethabi::Contract;
use web3::contract::Options;
use web3::types::{Address, U64};

use zksync_types::{TransactionReceipt, H160, H256, U256};

pub struct MultiPlexClient {
    clients: Vec<(String, Box<dyn ETHClientSender>)>,
    contract: ethabi::Contract,
}

impl MultiPlexClient {
    pub fn new(contract: ethabi::Contract) -> Self {
        Self {
            clients: vec![],
            contract,
        }
    }

    pub fn add_client<T: 'static + ETHClientSender>(mut self, name: String, client: T) -> Self {
        self.clients.push((name, Box::new(client)));
        self
    }
}

#[async_trait::async_trait]
impl ETHClientSender for MultiPlexClient {
    async fn pending_nonce(&self) -> Result<U256, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.pending_nonce().await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    async fn current_nonce(&self) -> Result<U256, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.current_nonce().await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    async fn block_number(&self) -> Result<U64, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.block_number().await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    async fn get_gas_price(&self) -> Result<U256, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.get_gas_price().await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    async fn balance(&self) -> Result<U256, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.balance().await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    async fn sign_prepared_tx(
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

    async fn sign_prepared_tx_for_addr(
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

    async fn send_raw_tx(&self, tx: Vec<u8>) -> Result<H256, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.send_raw_tx(tx.clone()).await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    async fn tx_receipt(&self, tx_hash: H256) -> Result<Option<TransactionReceipt>, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.tx_receipt(tx_hash).await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    async fn failure_reason(&self, tx_hash: H256) -> Result<Option<FailureInfo>, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.failure_reason(tx_hash).await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    async fn eth_balance(&self, address: Address) -> Result<U256, anyhow::Error> {
        for (name, client) in self.clients.iter() {
            match client.eth_balance(address).await {
                Ok(res) => return Ok(res),
                Err(err) => log::error!("Error in interface: {}, {} ", name, err),
            }
        }
        anyhow::bail!("All interfaces was wrong please try again")
    }

    async fn contract_balance(
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

    async fn allowance(
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
}

impl ETHTxEncoder for MultiPlexClient {
    fn contract(&self) -> &Contract {
        &self.contract
    }
}
