use crate::external_commands::js_revert_reason;

use anyhow::{bail, ensure, format_err};
use ethabi::ParamType;
use num::{BigUint, ToPrimitive};
use std::convert::TryFrom;
use std::str::FromStr;
use web3::api::Eth;
use web3::contract::{Contract, Options};
use web3::types::{
    BlockId, CallRequest, Transaction, TransactionId, TransactionReceipt, H256, U128, U256, U64,
};
use web3::{Transport, Web3};
use zksync_contracts::{erc20_contract, zksync_contract};
use zksync_crypto::proof::EncodedProofPlonk;
use zksync_eth_client::ETHClient;
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::block::Block;
use zksync_types::{AccountId, Address, Nonce, PriorityOp, PubKeyHash, TokenId};

pub fn parse_ether(eth_value: &str) -> Result<BigUint, anyhow::Error> {
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

    Ok(BigUint::from_str(&string_wei_value)?)
}

/// Used to sign and post ETH transactions for the zkSync contracts.
#[derive(Debug, Clone)]
pub struct EthereumAccount<T: Transport> {
    pub private_key: H256,
    pub address: Address,
    pub main_contract_eth_client: ETHClient<T, PrivateKeySigner>,
}

fn big_dec_to_u256(bd: BigUint) -> U256 {
    U256::from_dec_str(&bd.to_string()).unwrap()
}

fn u256_to_big_dec(u256: U256) -> BigUint {
    BigUint::from_str(&u256.to_string()).unwrap()
}

fn priority_op_from_tx_logs(receipt: &TransactionReceipt) -> Option<PriorityOp> {
    receipt
        .logs
        .iter()
        .find_map(|op| PriorityOp::try_from(op.clone()).ok())
}

impl<T: Transport> EthereumAccount<T> {
    pub fn new(
        private_key: H256,
        address: Address,
        transport: T,
        contract_address: Address,
        chain_id: u8,
        gas_price_factor: f64,
    ) -> Self {
        let eth_signer = PrivateKeySigner::new(private_key);
        let main_contract_eth_client = ETHClient::new(
            transport,
            zksync_contract(),
            address,
            eth_signer,
            contract_address,
            chain_id,
            gas_price_factor,
        );

        Self {
            private_key,
            address,
            main_contract_eth_client,
        }
    }

    pub async fn total_blocks_committed(&self) -> Result<u64, anyhow::Error> {
        let contract = Contract::new(
            self.main_contract_eth_client.web3.eth(),
            self.main_contract_eth_client.contract_addr,
            self.main_contract_eth_client.contract.clone(),
        );

        contract
            .query("totalBlocksCommitted", (), None, default_tx_options(), None)
            .await
            .map_err(|e| format_err!("Contract query fail: {}", e))
    }

    pub async fn total_blocks_verified(&self) -> Result<u64, anyhow::Error> {
        let contract = Contract::new(
            self.main_contract_eth_client.web3.eth(),
            self.main_contract_eth_client.contract_addr,
            self.main_contract_eth_client.contract.clone(),
        );

        contract
            .query("totalBlocksVerified", (), None, default_tx_options(), None)
            .await
            .map_err(|e| format_err!("Contract query fail: {}", e))
    }

    pub async fn is_exodus(&self) -> Result<bool, anyhow::Error> {
        let contract = Contract::new(
            self.main_contract_eth_client.web3.eth(),
            self.main_contract_eth_client.contract_addr,
            self.main_contract_eth_client.contract.clone(),
        );

        contract
            .query("exodusMode", (), None, default_tx_options(), None)
            .await
            .map_err(|e| format_err!("Contract query fail: {}", e))
    }

    pub async fn full_exit(
        &self,
        account_id: AccountId,
        token_address: Address,
    ) -> Result<(TransactionReceipt, PriorityOp), anyhow::Error> {
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "fullExit",
                (u64::from(account_id), token_address),
                default_tx_options(),
            )
            .await
            .map_err(|e| format_err!("Full exit send err: {}", e))?;
        let eth = self.main_contract_eth_client.web3.eth();
        let receipt = send_raw_tx_wait_confirmation(eth, signed_tx.raw_tx).await?;
        ensure!(
            receipt.status == Some(U64::from(1)),
            "Full exit submit fail"
        );
        Ok((
            receipt.clone(),
            priority_op_from_tx_logs(&receipt).expect("no priority op log in full exit"),
        ))
    }

    pub async fn exit(
        &self,
        account_id: AccountId,
        token_id: TokenId,
        amount: &BigUint,
        proof: EncodedProofPlonk,
    ) -> Result<ETHExecResult, anyhow::Error> {
        let mut options = Options::default();
        options.gas = Some(3_000_000.into()); // `exit` function requires more gas to operate.

        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "exit",
                (
                    u64::from(account_id),
                    u64::from(token_id),
                    U128::from(amount.to_u128().unwrap()),
                    proof.proof,
                ),
                options,
            )
            .await
            .map_err(|e| format_err!("Exit send err: {}", e))?;

        let eth = self.main_contract_eth_client.web3.eth();
        let receipt = send_raw_tx_wait_confirmation(eth, signed_tx.raw_tx).await?;

        Ok(ETHExecResult::new(receipt, &self.main_contract_eth_client.web3).await)
    }

    pub async fn cancel_outstanding_deposits_for_exodus_mode(
        &self,
        number: u64,
    ) -> Result<ETHExecResult, anyhow::Error> {
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "cancelOutstandingDepositsForExodusMode",
                number,
                default_tx_options(),
            )
            .await
            .map_err(|e| format_err!("cancelOutstandingDepositsForExodusMode send err: {}", e))?;

        let eth = self.main_contract_eth_client.web3.eth();
        let receipt = send_raw_tx_wait_confirmation(eth, signed_tx.raw_tx).await?;

        Ok(ETHExecResult::new(receipt, &self.main_contract_eth_client.web3).await)
    }

    pub async fn change_pubkey_priority_op(
        &self,
        new_pubkey_hash: &PubKeyHash,
    ) -> Result<PriorityOp, anyhow::Error> {
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "changePubKeyHash",
                (new_pubkey_hash.data.to_vec(),),
                default_tx_options(),
            )
            .await
            .map_err(|e| format_err!("ChangePubKeyHash send err: {}", e))?;
        let eth = self.main_contract_eth_client.web3.eth();
        let receipt = send_raw_tx_wait_confirmation(eth, signed_tx.raw_tx).await?;
        ensure!(
            receipt.status == Some(U64::from(1)),
            "ChangePubKeyHash transaction failed"
        );

        Ok(priority_op_from_tx_logs(&receipt).expect("no priority op log in change pubkey hash"))
    }

    /// Returns only one tx receipt. Return type is `Vec` for compatibility with deposit erc20
    pub async fn deposit_eth(
        &self,
        amount: BigUint,
        to: &Address,
        nonce: Option<U256>,
    ) -> Result<(Vec<TransactionReceipt>, PriorityOp), anyhow::Error> {
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "depositETH",
                *to,
                Options::with(|opt| {
                    opt.value = Some(big_dec_to_u256(amount.clone()));
                    opt.nonce = nonce;
                    opt.gas = Some(500_000.into());
                }),
            )
            .await
            .map_err(|e| format_err!("Deposit eth send err: {}", e))?;
        let eth = self.main_contract_eth_client.web3.eth();
        let receipt = send_raw_tx_wait_confirmation(eth, signed_tx.raw_tx).await?;
        ensure!(receipt.status == Some(U64::from(1)), "eth deposit fail");
        let priority_op =
            priority_op_from_tx_logs(&receipt).expect("no priority op log in deposit");
        Ok((vec![receipt], priority_op))
    }

    pub async fn eth_balance(&self) -> Result<BigUint, anyhow::Error> {
        Ok(u256_to_big_dec(
            self.main_contract_eth_client
                .web3
                .eth()
                .balance(self.address, None)
                .await?,
        ))
    }

    pub async fn erc20_balance(&self, token_contract: &Address) -> Result<BigUint, anyhow::Error> {
        let contract = Contract::new(
            self.main_contract_eth_client.web3.eth(),
            *token_contract,
            erc20_contract(),
        );
        contract
            .query("balanceOf", self.address, None, default_tx_options(), None)
            .await
            .map(u256_to_big_dec)
            .map_err(|e| format_err!("Contract query fail: {}", e))
    }

    pub async fn balances_to_withdraw(&self, token: TokenId) -> Result<BigUint, anyhow::Error> {
        let contract = Contract::new(
            self.main_contract_eth_client.web3.eth(),
            self.main_contract_eth_client.contract_addr,
            self.main_contract_eth_client.contract.clone(),
        );

        Ok(contract
            .query(
                "getBalanceToWithdraw",
                (self.address, u64::from(token)),
                None,
                default_tx_options(),
                None,
            )
            .await
            .map(u256_to_big_dec)
            .map_err(|e| format_err!("Contract query fail: {}", e))?)
    }

    pub async fn approve_erc20(
        &self,
        token_contract: Address,
        amount: BigUint,
    ) -> Result<TransactionReceipt, anyhow::Error> {
        let eth_signer = PrivateKeySigner::new(self.private_key);
        let erc20_client = ETHClient::new(
            self.main_contract_eth_client.web3.transport().clone(),
            erc20_contract(),
            self.address,
            eth_signer,
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
                default_tx_options(),
            )
            .await
            .map_err(|e| format_err!("Approve send err: {}", e))?;
        let eth = self.main_contract_eth_client.web3.eth();
        let receipt = send_raw_tx_wait_confirmation(eth, signed_tx.raw_tx).await?;

        ensure!(receipt.status == Some(U64::from(1)), "erc20 approve fail");

        Ok(receipt)
    }

    /// Returns TransactionReceipt of erc20 approve and erc20 deposit
    pub async fn deposit_erc20(
        &self,
        token_contract: Address,
        amount: BigUint,
        to: &Address,
    ) -> Result<(Vec<TransactionReceipt>, PriorityOp), anyhow::Error> {
        let approve_receipt = self.approve_erc20(token_contract, amount.clone()).await?;

        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "depositERC20",
                (token_contract, big_dec_to_u256(amount.clone()), *to),
                default_tx_options(),
            )
            .await
            .map_err(|e| format_err!("Deposit erc20 send err: {}", e))?;
        let eth = self.main_contract_eth_client.web3.eth();
        let receipt = send_raw_tx_wait_confirmation(eth, signed_tx.raw_tx).await?;
        let exec_result = ETHExecResult::new(receipt, &self.main_contract_eth_client.web3).await;
        let receipt = exec_result.success_result()?;
        let priority_op =
            priority_op_from_tx_logs(&receipt).expect("no priority op log in deposit erc20");
        Ok((vec![approve_receipt, receipt], priority_op))
    }

    pub async fn commit_block(&self, block: &Block) -> Result<ETHExecResult, anyhow::Error> {
        let witness_data = block.get_eth_witness_data();
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "commitBlock",
                (
                    u64::from(block.block_number),
                    u64::from(block.fee_account),
                    vec![block.get_eth_encoded_root()],
                    block.get_eth_public_data(),
                    witness_data.0,
                    witness_data.1,
                ),
                Options::with(|f| f.gas = Some(U256::from(9 * 10u64.pow(6)))),
            )
            .await
            .map_err(|e| format_err!("Commit block send err: {}", e))?;

        let eth = self.main_contract_eth_client.web3.eth();
        let receipt = send_raw_tx_wait_confirmation(eth, signed_tx.raw_tx).await?;

        Ok(ETHExecResult::new(receipt, &self.main_contract_eth_client.web3).await)
    }

    // Verifies block using provided proof or empty proof if None is provided. (`DUMMY_VERIFIER` should be enabled on the contract).
    pub async fn verify_block(
        &self,
        block: &Block,
        proof: Option<EncodedProofPlonk>,
    ) -> Result<ETHExecResult, anyhow::Error> {
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "verifyBlock",
                (
                    u64::from(block.block_number),
                    proof.unwrap_or_default().proof,
                    block.get_withdrawals_data(),
                ),
                Options::with(|f| f.gas = Some(U256::from(10 * 10u64.pow(6)))),
            )
            .await
            .map_err(|e| format_err!("Verify block send err: {}", e))?;
        let eth = self.main_contract_eth_client.web3.eth();
        let receipt = send_raw_tx_wait_confirmation(eth, signed_tx.raw_tx).await?;
        Ok(ETHExecResult::new(receipt, &self.main_contract_eth_client.web3).await)
    }

    // Completes pending withdrawals.
    pub async fn complete_withdrawals(&self) -> Result<ETHExecResult, anyhow::Error> {
        let max_withdrawals_to_complete: u64 = 999;
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "completeWithdrawals",
                max_withdrawals_to_complete,
                Options::with(|f| f.gas = Some(U256::from(9 * 10u64.pow(6)))),
            )
            .await
            .map_err(|e| format_err!("Complete withdrawals send err: {}", e))?;
        let eth = self.main_contract_eth_client.web3.eth();
        let receipt = send_raw_tx_wait_confirmation(eth, signed_tx.raw_tx).await?;

        Ok(ETHExecResult::new(receipt, &self.main_contract_eth_client.web3).await)
    }

    pub async fn revert_blocks(
        &self,
        blocks_to_revert: u64,
    ) -> Result<ETHExecResult, anyhow::Error> {
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "revertBlocks",
                blocks_to_revert,
                Options::with(|f| f.gas = Some(U256::from(9 * 10u64.pow(6)))),
            )
            .await
            .map_err(|e| format_err!("Revert blocks send err: {}", e))?;
        let eth = self.main_contract_eth_client.web3.eth();
        let receipt = send_raw_tx_wait_confirmation(eth, signed_tx.raw_tx).await?;

        Ok(ETHExecResult::new(receipt, &self.main_contract_eth_client.web3).await)
    }

    pub async fn trigger_exodus_if_needed(&self) -> Result<ETHExecResult, anyhow::Error> {
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx("triggerExodusIfNeeded", (), default_tx_options())
            .await
            .map_err(|e| format_err!("Trigger exodus if needed send err: {}", e))?;
        let eth = self.main_contract_eth_client.web3.eth();
        let receipt = send_raw_tx_wait_confirmation(eth, signed_tx.raw_tx).await?;

        Ok(ETHExecResult::new(receipt, &self.main_contract_eth_client.web3).await)
    }

    pub async fn eth_block_number(&self) -> Result<u64, anyhow::Error> {
        Ok(self.main_contract_eth_client.block_number().await?.as_u64())
    }

    pub async fn auth_fact(
        &self,
        fact: &[u8],
        nonce: Nonce,
    ) -> Result<TransactionReceipt, anyhow::Error> {
        let signed_tx = self
            .main_contract_eth_client
            .sign_call_tx(
                "setAuthPubkeyHash",
                (fact.to_vec(), u64::from(nonce)),
                default_tx_options(),
            )
            .await
            .map_err(|e| format_err!("AuthFact send err: {}", e))?;
        let eth = self.main_contract_eth_client.web3.eth();
        send_raw_tx_wait_confirmation(eth, signed_tx.raw_tx).await
    }
}

#[derive(Debug, Clone)]
pub struct ETHExecResult {
    success: bool,
    receipt: TransactionReceipt,
    revert_reason: String,
}

impl ETHExecResult {
    pub async fn new<T: Transport>(receipt: TransactionReceipt, web3: &Web3<T>) -> Self {
        let (success, revert_reason) = if receipt.status == Some(U64::from(1)) {
            (true, String::from(""))
        } else {
            let reason = get_revert_reason(&receipt, web3)
                .await
                .expect("Failed to get revert reason");
            (false, reason)
        };

        Self {
            success,
            revert_reason,
            receipt,
        }
    }

    pub fn success_result(self) -> Result<TransactionReceipt, anyhow::Error> {
        if self.success {
            Ok(self.receipt)
        } else {
            bail!(
                "revert reason: {}, tx: 0x{:x}",
                self.revert_reason,
                self.receipt.transaction_hash
            );
        }
    }

    pub fn expect_success(self) -> TransactionReceipt {
        let tx_hash = self.receipt.transaction_hash;
        self.success_result().unwrap_or_else(|e| {
            eprintln!("js revert reason:\n{}", js_revert_reason(&tx_hash));
            panic!("Expected transaction success: {}", e)
        })
    }

    pub fn expect_revert(self, code: &str) {
        if self.success {
            panic!(
                "Expected transaction fail, success instead, tx: 0x{:x}",
                self.receipt.transaction_hash
            );
        } else if self.revert_reason != code {
            panic!("Transaction failed with incorrect return code, expected: {}, found: {}, tx: 0x{:x}", code, self.revert_reason, self.receipt.transaction_hash);
        }
    }
}

/// Gets revert reason of failed transactions (i.e. if contract executes `require(false, "msg")` this function returns "msg")
async fn get_revert_reason<T: Transport>(
    receipt: &TransactionReceipt,
    web3: &Web3<T>,
) -> Result<String, anyhow::Error> {
    let tx = web3
        .eth()
        .transaction(TransactionId::Hash(receipt.transaction_hash))
        .await?;
    if let Some(Transaction {
        from,
        to: Some(to),
        gas,
        gas_price,
        value,
        input,
        ..
    }) = tx
    {
        // To get revert reason we have to make call to contract using the same args as function.
        let encoded_revert_reason = web3
            .eth()
            .call(
                CallRequest {
                    from: Some(from),
                    to: Some(to),
                    gas: Some(gas),
                    gas_price: Some(gas_price),
                    value: Some(value),
                    data: Some(input),
                },
                receipt.block_number.clone().map(BlockId::from),
            )
            .await?;

        // For some strange reason this could happen
        if encoded_revert_reason.0.len() < 4 {
            return Ok("".to_string());
        }
        // This function returns ABI encoded retrun value for function with signature "Error(string)"
        // we strip first 4 bytes because they encode function name "Error", the rest is encoded string.
        let encoded_string_without_function_hash = &encoded_revert_reason.0[4..];
        Ok(
            ethabi::decode(&[ParamType::String], encoded_string_without_function_hash)
                .map_err(|e| format_err!("ABI decode error {}", e))?
                .into_iter()
                .next()
                .unwrap()
                .to_string()
                .unwrap(),
        )
    } else {
        Ok("".to_string())
    }
}

async fn send_raw_tx_wait_confirmation<T: Transport>(
    eth: Eth<T>,
    raw_tx: Vec<u8>,
) -> Result<TransactionReceipt, anyhow::Error> {
    let tx_hash = eth
        .send_raw_transaction(raw_tx.into())
        .await
        .map_err(|e| format_err!("Failed to send raw tx: {}", e))?;
    loop {
        if let Some(receipt) = eth
            .transaction_receipt(tx_hash)
            .await
            .map_err(|e| format_err!("Failed to get receipt from eth node: {}", e))?
        {
            return Ok(receipt);
        }
    }
}

fn default_tx_options() -> Options {
    let mut options = Options::default();
    // Set the gas limit, so `eth_client` won't complain about it.
    options.gas = Some(500_000.into());

    options
}

/// Get fee paid in wei for tx execution
pub async fn get_executed_tx_fee<T: Transport>(
    eth: Eth<T>,
    receipt: &TransactionReceipt,
) -> Result<BigUint, anyhow::Error> {
    let gas_used = receipt.gas_used.ok_or_else(|| {
        format_err!(
            "Not used gas in the receipt: 0x{:x?}",
            receipt.transaction_hash
        )
    })?;

    let tx = eth
        .transaction(TransactionId::Hash(receipt.transaction_hash))
        .await?
        .ok_or_else(|| format_err!("Transaction not found: 0x{:x?}", receipt.transaction_hash))?;

    Ok((gas_used * tx.gas_price).to_string().parse().unwrap())
}
