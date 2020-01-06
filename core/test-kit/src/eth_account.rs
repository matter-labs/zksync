use bigdecimal::BigDecimal;
use eth_client::ETHClient;
use failure::{ensure, format_err};
use futures::compat::Future01CompatExt;
use models::abi::FRANKLIN_CONTRACT;
use models::node::block::Block;
use models::node::{AccountAddress, PriorityOp, TokenId, U128};
use server::ConfigurationOptions;
use std::convert::TryFrom;
use std::str::FromStr;
use std::time::Duration;
use web3::contract::Options;
use web3::types::{Address, TransactionReceipt, H256, U256};
use web3::Transport;

pub fn parse_ether(eth_value: &str) -> Result<BigDecimal, failure::Error> {
    let split = eth_value.split(".").collect::<Vec<&str>>();
    ensure!(split.len() == 1 || split.len() == 2, "Wrong eth value");
    let string_wei_value = if split.len() == 1 {
        format!("{}000000000000000000", split[0])
    } else if split.len() == 2 {
        let before_dot = split[0];
        let after_dot = split[1];
        ensure!(
            after_dot.len() <= 18,
            "ETH value can have up to 18 digits after dot."
        );
        let zeros_to_pad = 18 - after_dot.len();
        format!("{}{}{}", before_dot, after_dot, "0".repeat(zeros_to_pad))
    } else {
        unreachable!()
    };

    Ok(BigDecimal::from_str(&string_wei_value)?)
}

fn priority_op_fee() -> BigDecimal {
    parse_ether("0.3").unwrap()
}

pub struct EthereumAccount<T: Transport> {
    pub private_key: H256,
    pub address: Address,
    pub main_contract_eth_client: ETHClient<T>,
}

fn big_dec_to_u256(bd: BigDecimal) -> U256 {
    U256::from_dec_str(&bd.to_string()).unwrap()
}

fn u256_to_big_dec(u256: U256) -> BigDecimal {
    BigDecimal::from_str(&u256.to_string()).unwrap()
}

impl<T: Transport> EthereumAccount<T> {
    pub fn new(
        private_key: H256,
        address: Address,
        transport: T,
        config: &ConfigurationOptions,
    ) -> Self {
        let abi_string = serde_json::Value::from_str(FRANKLIN_CONTRACT)
            .unwrap()
            .get("abi")
            .unwrap()
            .to_string();
        let main_contract_eth_client = ETHClient::new(
            transport,
            abi_string,
            address.clone(),
            private_key.clone(),
            config.contract_eth_addr.clone(),
            config.chain_id,
            config.gas_price_factor,
        );

        Self {
            private_key,
            address,
            main_contract_eth_client,
        }
    }

    pub async fn deposit_eth(
        &self,
        amount: BigDecimal,
        fee: BigDecimal,
        to: &AccountAddress,
    ) -> Result<PriorityOp, failure::Error> {
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "depositETH",
                (big_dec_to_u256(amount.clone()), to.data.to_vec()),
                Options::with(|opt| {
                    opt.value = Some(big_dec_to_u256(amount.clone() + priority_op_fee()))
                }),
            )
            .await
            .map_err(|e| format_err!("Deposit send err: {}", e))?;
        let receipt = self
            .main_contract_eth_client
            .web3
            .send_raw_transaction_with_confirmation(
                signed_tx.raw_tx.into(),
                Duration::from_millis(500),
                1,
            )
            .compat()
            .await
            .map_err(|e| format_err!("Deposit wait confirm err: {}", e))?;
        Ok(receipt
            .logs
            .into_iter()
            .map(PriorityOp::try_from)
            .filter_map(|op| op.ok())
            .next()
            .expect("no priority op log in deposit"))
    }

    pub async fn eth_balance(&self) -> Result<BigDecimal, failure::Error> {
        Ok(u256_to_big_dec(
            self.main_contract_eth_client
                .web3
                .eth()
                .balance(self.address.clone(), None)
                .compat()
                .await?,
        ))
    }

    pub async fn commit_block(&self, block: &Block) -> Result<TransactionReceipt, failure::Error> {
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "commitBlock",
                (
                    u64::from(block.block_number),
                    u64::from(block.fee_account),
                    block.get_eth_encoded_root(),
                    block.get_eth_public_data(),
                ),
                Options::default(),
            )
            .await
            .map_err(|e| format_err!("Commit block send err: {}", e))?;
        Ok(self
            .main_contract_eth_client
            .web3
            .send_raw_transaction_with_confirmation(
                signed_tx.raw_tx.into(),
                Duration::from_millis(500),
                1,
            )
            .compat()
            .await
            .map_err(|e| format_err!("Commit block confirm err: {}", e))?)
    }

    pub async fn verify_block(&self, block: &Block) -> Result<TransactionReceipt, failure::Error> {
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "verifyBlock",
                (u64::from(block.block_number), [U256::default(); 8]),
                Options::default(),
            )
            .await
            .map_err(|e| format_err!("Verify block send err: {}", e))?;
        Ok(self
            .main_contract_eth_client
            .web3
            .send_raw_transaction_with_confirmation(
                signed_tx.raw_tx.into(),
                Duration::from_millis(500),
                1,
            )
            .compat()
            .await
            .map_err(|e| format_err!("Verify block confirm err: {}", e))?)
    }
}
