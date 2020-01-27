use crate::external_commands::get_revert_reason;
use bigdecimal::BigDecimal;
use eth_client::ETHClient;
use failure::{ensure, format_err};
use futures::compat::Future01CompatExt;
use futures::future::IntoFuture;
use models::abi::{erc20_contract, zksync_contract};
use models::config_options::ConfigurationOptions;
use models::node::block::Block;
use models::node::{AccountAddress, AccountId, Nonce, TokenId, PriorityOp};
use models::EncodedProof;
use std::convert::TryFrom;
use std::str::FromStr;
use std::time::Duration;
use web3::contract::{Contract, Options};
use web3::contract::tokens::{Tokenize, Tokenizable};
use web3::types::{Address, TransactionReceipt, H256, U256, U128, U64};
use web3::Transport;
use web3::Error;
pub fn parse_ether(eth_value: &str) -> Result<BigDecimal, failure::Error> {
    let split = eth_value.split('.').collect::<Vec<&str>>();
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

/// Used to sign and post ETH transactions for the ZK Sync contracts.
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
        contract_address: Address,
        config: &ConfigurationOptions,
    ) -> Self {
        let main_contract_eth_client = ETHClient::new(
            transport,
            zksync_contract(),
            address,
            private_key,
            contract_address,
            config.chain_id,
            config.gas_price_factor,
        );

        Self {
            private_key,
            address,
            main_contract_eth_client,
        }
    }

    pub async fn is_exodus(&self) -> Result<bool, failure::Error> {
        let contract = Contract::new(
            self.main_contract_eth_client.web3.eth(),
            self.main_contract_eth_client.contract_addr.clone(),
            self.main_contract_eth_client.contract.clone()
        );

        // let result = contract.query("exodusMode", (), None, Options::default(), None);
        // let whatever = result.into_future().await.unwrap()


        Ok(false)
        // self
        //     .main_contract_eth_client
        //     .contract
        //     .query(
        //         "exodusMode",
        //         (),
        //         None,
        //         Options::default(),
        //         None
        //     )
        //     .await
        //     .map_err(|e| format_err!("Is exodus query err: {}", e))
    }

    pub async fn full_exit(
        &self,
        account_id: AccountId,
        pub_key: &[u8],
        token_address: Address,
        signature: &[u8],
        nonce: Nonce,
    ) -> Result<PriorityOp, failure::Error> {
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "fullExit",
                (
                    u64::from(account_id),
                    pub_key.to_vec(),
                    token_address,
                    signature.to_vec(),
                    u64::from(nonce),
                ),
                Options::with(|opt| opt.value = Some(big_dec_to_u256(priority_op_fee()))),
            )
            .await
            .map_err(|e| format_err!("Full exit send err: {}", e))?;
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
            .map_err(|e| format_err!("Full exit wait confirm err: {}", e))?;
        ensure!(
            receipt.status == Some(U64::from(1)),
            "Full exit submit fail"
        );
        Ok(receipt
            .logs
            .into_iter()
            .map(PriorityOp::try_from)
            .filter_map(|op| op.ok())
            .next()
            .expect("no priority op log in full exit"))
    }

    pub async fn exit(
        &self,
        tokenId: TokenId,
        owner: Address,
        amount: u128,
        proof: EncodedProof,
    ) -> Result<String, failure::Error> {
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "exit",
                (
                    u64::from(tokenId),
                    owner,
                    U128::from(amount),
                    proof,
                ),
                Options::default(),
            )
            .await
            .map_err(|e| format_err!("Exit send err: {}", e))?;
        
        let receipt: web3::types::TransactionReceipt = self
            .main_contract_eth_client
            .web3
            .send_raw_transaction_with_confirmation(
                signed_tx.raw_tx.into(),
                Duration::from_millis(500),
                1,
            )
            .compat()
            .await
            .map_err(|e| format_err!("Exit wait confirm err: {}", e))?;

        let hash = format!("{:#?}", &receipt.transaction_hash);
        let reason = get_revert_reason(&hash);
        println!(
            "hash: {}, reason: {}",
            &hash,
            &reason
        );

        ensure!(
            receipt.status == Some(U64::from(1)),
            "Exit submit fail"
        );

        Ok(reason)
        // Ok(receipt
        //     .logs
        //     .into_iter()
        //     .map(PriorityOp::try_from)
        //     .filter_map(|op| op.ok())
        //     .next()
        //     .expect("no priority op log in full exit"))
    }

    pub async fn deposit_eth(
        &self,
        amount: BigDecimal,
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
            .map_err(|e| format_err!("Deposit eth send err: {}", e))?;
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
            .map_err(|e| format_err!("Deposit eth wait confirm err: {}", e))?;
        ensure!(receipt.status == Some(U64::from(1)), "eth deposit fail");
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

    pub async fn erc20_balance(
        &self,
        token_contract: &Address,
    ) -> Result<BigDecimal, failure::Error> {
        let contract = Contract::new(
            self.main_contract_eth_client.web3.eth(),
            *token_contract,
            erc20_contract(),
        );
        contract
            .query("balanceOf", self.address, None, Options::default(), None)
            .compat()
            .await
            .map(u256_to_big_dec)
            .map_err(|e| format_err!("Contract query fail: {}", e))
    }

    pub async fn approve_erc20(
        &self,
        token_contract: Address,
        amount: BigDecimal,
    ) -> Result<(), failure::Error> {
        let erc20_client = ETHClient::new(
            self.main_contract_eth_client.web3.transport().clone(),
            erc20_contract(),
            self.address,
            self.private_key,
            token_contract,
            self.main_contract_eth_client.chain_id,
            self.main_contract_eth_client.gas_price_factor,
        );

        let signed_tx = erc20_client
            .sign_call_tx(
                "approve",
                (
                    self.main_contract_eth_client.contract_addr,
                    big_dec_to_u256(amount.clone()),
                ),
                Options::default(),
            )
            .await
            .map_err(|e| format_err!("Approve send err: {}", e))?;
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
            .map_err(|e| format_err!("Approve wait confirm err: {}", e))?;

        ensure!(receipt.status == Some(U64::from(1)), "erc20 approve fail");

        Ok(())
    }

    pub async fn deposit_erc20(
        &self,
        token_contract: Address,
        amount: BigDecimal,
        to: &AccountAddress,
    ) -> Result<PriorityOp, failure::Error> {
        self.approve_erc20(token_contract, amount.clone()).await?;

        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "depositERC20",
                (
                    token_contract,
                    big_dec_to_u256(amount.clone()),
                    to.data.to_vec(),
                ),
                Options::with(|opt| opt.value = Some(big_dec_to_u256(priority_op_fee()))),
            )
            .await
            .map_err(|e| format_err!("Deposit erc20 send err: {}", e))?;
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
            .map_err(|e| format_err!("Deposit erc20 wait confirm err: {}", e))?;
        ensure!(receipt.status == Some(U64::from(1)), "erc20 deposit fail");
        Ok(receipt
            .logs
            .into_iter()
            .map(PriorityOp::try_from)
            .filter_map(|op| op.ok())
            .next()
            .expect("no priority op log in deposit"))
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

    // Verifies block using empty proof. (`DUMMY_VERIFIER` should be enabled on the contract).
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
