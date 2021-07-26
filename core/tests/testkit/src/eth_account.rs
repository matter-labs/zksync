use crate::external_commands::js_revert_reason;

use anyhow::{bail, ensure, format_err};
use ethabi::Token;
use num::{BigUint, ToPrimitive};
use std::convert::TryFrom;
use std::str::FromStr;
use web3::{
    contract::Options,
    transports::Http,
    types::{TransactionReceipt, H256, U128, U256, U64},
};
use zksync_contracts::{erc20_contract, zksync_contract};
use zksync_crypto::proof::EncodedSingleProof;
use zksync_eth_client::ETHDirectClient;
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::aggregated_operations::{
    stored_block_info, BlocksCommitOperation, BlocksExecuteOperation, BlocksProofOperation,
};
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
pub struct EthereumAccount {
    pub private_key: H256,
    pub address: Address,
    pub main_contract_eth_client: ETHDirectClient<PrivateKeySigner>,
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

impl EthereumAccount {
    pub fn new(
        private_key: H256,
        address: Address,
        transport: Http,
        contract_address: Address,
        chain_id: u8,
        gas_price_factor: f64,
    ) -> Self {
        let eth_signer = PrivateKeySigner::new(private_key);
        let main_contract_eth_client = ETHDirectClient::new(
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
        let contract = self.main_contract_eth_client.main_contract();
        contract
            .query("totalBlocksCommitted", (), None, default_tx_options(), None)
            .await
            .map_err(|e| format_err!("Contract query fail: {}", e))
    }

    pub async fn total_blocks_verified(&self) -> Result<u64, anyhow::Error> {
        let contract = self.main_contract_eth_client.main_contract();

        contract
            .query("totalBlocksVerified", (), None, default_tx_options(), None)
            .await
            .map_err(|e| format_err!("Contract query fail: {}", e))
    }

    pub async fn is_exodus(&self) -> Result<bool, anyhow::Error> {
        let contract = self.main_contract_eth_client.main_contract();

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
        let data = self
            .main_contract_eth_client
            .encode_tx_data("requestFullExit", (u64::from(*account_id), token_address));

        let signed_tx = self
            .main_contract_eth_client
            .sign_prepared_tx(data, default_tx_options())
            .await
            .map_err(|e| format_err!("Full exit send err: {}", e))?;
        let receipt =
            send_raw_tx_wait_confirmation(&self.main_contract_eth_client, signed_tx.raw_tx).await?;
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
        last_block: &Block,
        account_id: AccountId,
        token_id: TokenId,
        amount: &BigUint,
        zero_account_address: Address,
        proof: EncodedSingleProof,
    ) -> Result<ETHExecResult, anyhow::Error> {
        let options = Options {
            gas: Some(3_000_000.into()),
            // `exit` function requires more gas to operate.
            ..Default::default()
        };

        let stored_block_info = stored_block_info(last_block);
        let data = self.main_contract_eth_client.encode_tx_data(
            "performExodus",
            (
                stored_block_info,
                self.address,
                u64::from(*account_id),
                u64::from(*token_id),
                U128::from(amount.to_u128().unwrap()),
                0u64,
                zero_account_address,
                0u64,
                H256::default(),
                proof.proof,
            ),
        );
        let signed_tx = self
            .main_contract_eth_client
            .sign_prepared_tx(data, options)
            .await
            .map_err(|e| format_err!("Exit send err: {}", e))?;

        let receipt =
            send_raw_tx_wait_confirmation(&self.main_contract_eth_client, signed_tx.raw_tx).await?;

        Ok(ETHExecResult::new(receipt, &self.main_contract_eth_client).await)
    }

    pub async fn cancel_outstanding_deposits_for_exodus_mode(
        &self,
        number: u64,
        priority_op_data: Vec<Vec<u8>>,
    ) -> Result<ETHExecResult, anyhow::Error> {
        let data = self.main_contract_eth_client.encode_tx_data(
            "cancelOutstandingDepositsForExodusMode",
            (number, priority_op_data),
        );
        let signed_tx = self
            .main_contract_eth_client
            .sign_prepared_tx(data, default_tx_options())
            .await
            .map_err(|e| format_err!("cancelOutstandingDepositsForExodusMode send err: {}", e))?;

        let receipt =
            send_raw_tx_wait_confirmation(&self.main_contract_eth_client, signed_tx.raw_tx).await?;

        Ok(ETHExecResult::new(receipt, &self.main_contract_eth_client).await)
    }

    pub async fn change_pubkey_priority_op(
        &self,
        new_pubkey_hash: &PubKeyHash,
    ) -> Result<PriorityOp, anyhow::Error> {
        let data = self
            .main_contract_eth_client
            .encode_tx_data("changePubKeyHash", (new_pubkey_hash.data.to_vec(),));
        let signed_tx = self
            .main_contract_eth_client
            .sign_prepared_tx(data, default_tx_options())
            .await
            .map_err(|e| format_err!("ChangePubKeyHash send err: {}", e))?;
        let receipt =
            send_raw_tx_wait_confirmation(&self.main_contract_eth_client, signed_tx.raw_tx).await?;
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
        let data = self
            .main_contract_eth_client
            .encode_tx_data("depositETH", *to);
        let signed_tx = self
            .main_contract_eth_client
            .sign_prepared_tx(
                data,
                Options::with(|opt| {
                    opt.value = Some(big_dec_to_u256(amount.clone()));
                    opt.nonce = nonce;
                    opt.gas = Some(500_000.into());
                }),
            )
            .await
            .map_err(|e| format_err!("Deposit eth send err: {}", e))?;
        let receipt =
            send_raw_tx_wait_confirmation(&self.main_contract_eth_client, signed_tx.raw_tx).await?;
        ensure!(receipt.status == Some(U64::from(1)), "eth deposit fail");
        let priority_op =
            priority_op_from_tx_logs(&receipt).expect("no priority op log in deposit");
        Ok((vec![receipt], priority_op))
    }

    pub async fn eth_balance(&self) -> Result<BigUint, anyhow::Error> {
        Ok(u256_to_big_dec(
            self.main_contract_eth_client
                .eth_balance(self.address)
                .await?,
        ))
    }

    pub async fn erc20_balance(&self, token_contract: &Address) -> Result<BigUint, anyhow::Error> {
        self.main_contract_eth_client
            .call_contract_function(
                "balanceOf",
                self.address,
                None,
                Options::default(),
                None,
                *token_contract,
                erc20_contract(),
            )
            .await
            .map(u256_to_big_dec)
            .map_err(|e| format_err!("Contract query fail: {}", e))
    }

    pub async fn balances_to_withdraw(&self, token: Address) -> Result<BigUint, anyhow::Error> {
        let contract = self.main_contract_eth_client.main_contract();

        Ok(contract
            .query(
                "getPendingBalance",
                (self.address, token),
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
        let erc20_client = ETHDirectClient::new(
            self.main_contract_eth_client.get_web3_transport().clone(),
            erc20_contract(),
            self.address,
            eth_signer,
            token_contract,
            self.main_contract_eth_client.chain_id(),
            self.main_contract_eth_client.gas_price_factor(),
        );
        let data = erc20_client.encode_tx_data(
            "approve",
            (
                self.main_contract_eth_client.contract_addr(),
                big_dec_to_u256(amount.clone()),
            ),
        );

        let signed_tx = erc20_client
            .sign_prepared_tx(data, default_tx_options())
            .await
            .map_err(|e| format_err!("Approve send err: {}", e))?;
        let receipt =
            send_raw_tx_wait_confirmation(&self.main_contract_eth_client, signed_tx.raw_tx).await?;

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

        let data = self.main_contract_eth_client.encode_tx_data(
            "depositERC20",
            (token_contract, big_dec_to_u256(amount.clone()), *to),
        );
        let signed_tx = self
            .main_contract_eth_client
            .sign_prepared_tx(data, default_tx_options())
            .await
            .map_err(|e| format_err!("Deposit erc20 send err: {}", e))?;
        let receipt =
            send_raw_tx_wait_confirmation(&self.main_contract_eth_client, signed_tx.raw_tx).await?;
        let exec_result = ETHExecResult::new(receipt, &self.main_contract_eth_client).await;
        let receipt = exec_result.success_result()?;
        let priority_op =
            priority_op_from_tx_logs(&receipt).expect("no priority op log in deposit erc20");
        Ok((vec![approve_receipt, receipt], priority_op))
    }

    pub async fn commit_block(
        &self,
        commit_operation: &BlocksCommitOperation,
    ) -> Result<ETHExecResult, anyhow::Error> {
        let data = self.main_contract_eth_client.encode_tx_data(
            "commitBlocks",
            commit_operation.get_eth_tx_args().as_slice(),
        );
        let signed_tx = self
            .main_contract_eth_client
            .sign_prepared_tx(
                data,
                Options::with(|f| f.gas = Some(U256::from(9 * 10u64.pow(6)))),
            )
            .await
            .map_err(|e| format_err!("Commit block send err: {}", e))?;

        let receipt =
            send_raw_tx_wait_confirmation(&self.main_contract_eth_client, signed_tx.raw_tx).await?;

        Ok(ETHExecResult::new(receipt, &self.main_contract_eth_client).await)
    }

    // Verifies block using provided proof or empty proof if None is provided. (`DUMMY_VERIFIER` should be enabled on the contract).
    pub async fn verify_block(
        &self,
        proof_operation: &BlocksProofOperation,
    ) -> Result<ETHExecResult, anyhow::Error> {
        let data = self
            .main_contract_eth_client
            .encode_tx_data("proveBlocks", proof_operation.get_eth_tx_args().as_slice());
        let signed_tx = self
            .main_contract_eth_client
            .sign_prepared_tx(
                data,
                Options::with(|f| f.gas = Some(U256::from(10 * 10u64.pow(6)))),
            )
            .await
            .map_err(|e| format_err!("Verify block send err: {}", e))?;
        let receipt =
            send_raw_tx_wait_confirmation(&self.main_contract_eth_client, signed_tx.raw_tx).await?;
        Ok(ETHExecResult::new(receipt, &self.main_contract_eth_client).await)
    }

    // Completes pending withdrawals.
    pub async fn execute_block(
        &self,
        execute_operation: &BlocksExecuteOperation,
    ) -> Result<ETHExecResult, anyhow::Error> {
        let data = self.main_contract_eth_client.encode_tx_data(
            "executeBlocks",
            execute_operation.get_eth_tx_args().as_slice(),
        );

        let signed_tx = self
            .main_contract_eth_client
            .sign_prepared_tx(
                data,
                Options::with(|f| f.gas = Some(U256::from(9 * 10u64.pow(6)))),
            )
            .await
            .map_err(|e| format_err!("Complete withdrawals send err: {}", e))?;
        let receipt =
            send_raw_tx_wait_confirmation(&self.main_contract_eth_client, signed_tx.raw_tx).await?;

        Ok(ETHExecResult::new(receipt, &self.main_contract_eth_client).await)
    }

    pub async fn revert_blocks(&self, blocks: &[Block]) -> Result<ETHExecResult, anyhow::Error> {
        let tx_arg = Token::Array(blocks.iter().map(stored_block_info).collect());

        let data = self
            .main_contract_eth_client
            .encode_tx_data("revertBlocks", tx_arg);

        let signed_tx = self
            .main_contract_eth_client
            .sign_prepared_tx(
                data,
                Options::with(|f| f.gas = Some(U256::from(9 * 10u64.pow(6)))),
            )
            .await
            .map_err(|e| format_err!("Revert blocks send err: {}", e))?;
        let receipt =
            send_raw_tx_wait_confirmation(&self.main_contract_eth_client, signed_tx.raw_tx).await?;

        Ok(ETHExecResult::new(receipt, &self.main_contract_eth_client).await)
    }

    pub async fn trigger_exodus_if_needed(&self) -> Result<ETHExecResult, anyhow::Error> {
        let data = self
            .main_contract_eth_client
            .encode_tx_data("activateExodusMode", ());
        let signed_tx = self
            .main_contract_eth_client
            .sign_prepared_tx(data, default_tx_options())
            .await
            .map_err(|e| format_err!("Trigger exodus if needed send err: {}", e))?;
        let receipt =
            send_raw_tx_wait_confirmation(&self.main_contract_eth_client, signed_tx.raw_tx).await?;

        Ok(ETHExecResult::new(receipt, &self.main_contract_eth_client).await)
    }

    pub async fn eth_block_number(&self) -> Result<u64, anyhow::Error> {
        Ok(self.main_contract_eth_client.block_number().await?.as_u64())
    }

    pub async fn auth_fact(
        &self,
        fact: &[u8],
        nonce: Nonce,
    ) -> Result<TransactionReceipt, anyhow::Error> {
        let data = self
            .main_contract_eth_client
            .encode_tx_data("setAuthPubkeyHash", (fact.to_vec(), u64::from(*nonce)));
        let signed_tx = self
            .main_contract_eth_client
            .sign_prepared_tx(data, default_tx_options())
            .await
            .map_err(|e| format_err!("AuthFact send err: {}", e))?;
        send_raw_tx_wait_confirmation(&self.main_contract_eth_client, signed_tx.raw_tx).await
    }
}

#[derive(Debug, Clone)]
pub struct ETHExecResult {
    success: bool,
    receipt: TransactionReceipt,
    revert_reason: String,
}

impl ETHExecResult {
    pub async fn new(
        receipt: TransactionReceipt,
        client: &ETHDirectClient<PrivateKeySigner>,
    ) -> Self {
        let (success, revert_reason) = if receipt.status == Some(U64::from(1)) {
            (true, String::from(""))
        } else {
            let reason = client
                .failure_reason(receipt.transaction_hash)
                .await
                .expect("Failed to get revert reason")
                .unwrap()
                .revert_reason;
            (false, reason)
        };

        Self {
            success,
            receipt,
            revert_reason,
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

async fn send_raw_tx_wait_confirmation(
    client: &ETHDirectClient<PrivateKeySigner>,
    raw_tx: Vec<u8>,
) -> Result<TransactionReceipt, anyhow::Error> {
    let tx_hash = client
        .send_raw_tx(raw_tx)
        .await
        .map_err(|e| format_err!("Failed to send raw tx: {}", e))?;
    loop {
        if let Some(receipt) = client
            .tx_receipt(tx_hash)
            .await
            .map_err(|e| format_err!("Failed to get receipt from eth node: {}", e))?
        {
            return Ok(receipt);
        }
    }
}

fn default_tx_options() -> Options {
    // Set the gas limit, so `eth_client` won't complain about it.
    Options {
        gas: Some(500_000.into()),
        ..Default::default()
    }
}

/// Get fee paid in wei for tx execution
pub async fn get_executed_tx_fee(
    client: &ETHDirectClient<PrivateKeySigner>,
    receipt: &TransactionReceipt,
) -> Result<BigUint, anyhow::Error> {
    let gas_used = receipt.gas_used.ok_or_else(|| {
        format_err!(
            "Not used gas in the receipt: 0x{:x?}",
            receipt.transaction_hash
        )
    })?;

    let tx = client
        .get_tx(receipt.transaction_hash)
        .await?
        .ok_or_else(|| format_err!("Transaction not found: 0x{:x?}", receipt.transaction_hash))?;

    Ok((gas_used * tx.gas_price).to_string().parse().unwrap())
}
