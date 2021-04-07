use std::convert::TryInto;

use num::{BigUint, Zero};
use zksync::{
    error::ClientError, ethereum::PriorityOpHolder, operations::SyncTransactionHandle,
    provider::Provider,
};
use zksync_types::{tokens::ETH_TOKEN_ID, tx::PackedEthSignature, Nonce, ZkSyncTx, H256};

use crate::{
    account::AccountLifespan,
    command::{IncorrectnessModifier, TxCommand, TxType},
    constants::{COMMIT_TIMEOUT, POLLING_INTERVAL},
    corrupted_tx::Corrupted,
    report::ReportLabel,
};

impl AccountLifespan {
    pub(super) async fn execute_tx_command(
        &mut self,
        command: &TxCommand,
    ) -> Result<ReportLabel, ClientError> {
        match command.command_type {
            TxType::ChangePubKey => self.execute_change_pubkey(&command).await,
            TxType::TransferToExisting | TxType::TransferToNew => {
                self.execute_transfer(&command).await
            }
            TxType::WithdrawToOther | TxType::WithdrawToSelf => {
                self.execute_withdraw(&command).await
            }
            TxType::Deposit => self.execute_deposit(&command).await,
            TxType::FullExit => self.execute_full_exit().await,
        }
    }

    fn tx_creation_error(err: ClientError) -> ClientError {
        // Translate network errors (so operation will be retried), but don't accept other ones.
        // For example, we will retry operation if fee ticker returned an error,
        // but will panic if transaction cannot be signed.
        match err {
            ClientError::NetworkError(_)
            | ClientError::RpcError(_)
            | ClientError::MalformedResponse(_) => err,
            _ => panic!("Transaction should be correct"),
        }
    }

    fn apply_modifier(
        &self,
        tx: ZkSyncTx,
        eth_signature: Option<PackedEthSignature>,
        modifier: IncorrectnessModifier,
    ) -> (ZkSyncTx, Option<PackedEthSignature>) {
        (tx, eth_signature).apply_modifier(
            modifier,
            self.eth_pk,
            self.main_token.symbol.as_ref(),
            self.main_token.decimals,
        )
    }

    /// Returns the balances for ETH and the main token on the L1.
    /// This function is used to check whether the L1 operation can be performed or should be
    /// skipped.
    async fn l1_balances(&self) -> Result<(BigUint, BigUint), ClientError> {
        let ethereum = self.wallet.ethereum(&self.config.web3_url).await?;
        let eth_balance = ethereum.balance().await?;
        let erc20_balance = ethereum
            .erc20_balance(self.wallet.address(), self.main_token.id)
            .await?;

        // Casting via `low_u128` is safe here, since we don't use numbers higher than `u128::max_value()`.
        let erc20_balance = erc20_balance.low_u128().into();
        Ok((eth_balance, erc20_balance))
    }

    async fn execute_deposit(&self, command: &TxCommand) -> Result<ReportLabel, ClientError> {
        let (eth_balance, erc20_balance) = self.l1_balances().await?;
        if eth_balance.is_zero() || erc20_balance < command.amount {
            // We don't have either funds in L1 to pay for tx or to deposit.
            // It's not a problem with the server, thus we mark this operation as skipped.
            return Ok(ReportLabel::skipped("No L1 balance"));
        }

        let ethereum = self.wallet.ethereum(&self.config.web3_url).await?;

        // We should check whether we've previously approved ERC-20 deposits.
        let deposits_allowed = ethereum
            .is_erc20_deposit_approved(self.main_token.id)
            .await?;
        if !deposits_allowed {
            ethereum
                .approve_erc20_token_deposits(self.main_token.id)
                .await?;
        }

        // Convert BigUint into U256. We won't ever use values above `u128::max_value()`, but just in case we'll ever
        // met such a value, we'll truncate it to the limit.
        let amount = command
            .amount
            .clone()
            .try_into()
            .unwrap_or_else(|_| u128::max_value())
            .into();
        let eth_tx_hash = match ethereum
            .deposit(self.main_token.id, amount, self.wallet.address())
            .await
        {
            Ok(hash) => hash,
            Err(err) => {
                // Most likely we don't have enough ETH to perform operations.
                // Just mark the operations as skipped.
                let reason = format!("Unable to perform an L1 operation. Reason: {}", err);
                return Ok(ReportLabel::skipped(&reason));
            }
        };

        self.handle_priority_op(eth_tx_hash).await
    }

    async fn execute_full_exit(&self) -> Result<ReportLabel, ClientError> {
        let balances = self.l1_balances().await?;
        if balances.0.is_zero() {
            // We don't have either funds in L1 to pay for tx.
            return Ok(ReportLabel::skipped("No L1 balance"));
        }

        // We always call full exit for the ETH, since we don't want to leave the wallet without main token.
        let exit_token_id = ETH_TOKEN_ID;

        let account_id = match self.wallet.account_id() {
            Some(id) => id,
            None => {
                return Ok(ReportLabel::skipped("L2 account was not initialized yet"));
            }
        };

        let ethereum = self.wallet.ethereum(&self.config.web3_url).await?;
        let eth_tx_hash = match ethereum.full_exit(exit_token_id, account_id).await {
            Ok(hash) => hash,
            Err(_err) => {
                // Most likely we don't have enough ETH to perform operations.
                // Just mark the operations as skipped.
                return Ok(ReportLabel::skipped("Unable to perform an L1 operation"));
            }
        };

        self.handle_priority_op(eth_tx_hash).await
    }

    async fn handle_priority_op(&self, eth_tx_hash: H256) -> Result<ReportLabel, ClientError> {
        let ethereum = self.wallet.ethereum(&self.config.web3_url).await?;
        let receipt = ethereum.wait_for_tx(eth_tx_hash).await?;

        let mut priority_op_handle = match receipt.priority_op_handle(self.wallet.provider.clone())
        {
            Some(handle) => handle,
            None => {
                // Probably we did something wrong, no big deal.
                return Ok(ReportLabel::skipped(
                    "Ethereum transaction for deposit failed",
                ));
            }
        };

        priority_op_handle
            .polling_interval(POLLING_INTERVAL)
            .unwrap();
        priority_op_handle
            .commit_timeout(COMMIT_TIMEOUT)
            .wait_for_commit()
            .await?;

        Ok(ReportLabel::done())
    }

    async fn execute_change_pubkey(&self, command: &TxCommand) -> Result<ReportLabel, ClientError> {
        let (tx, eth_signature) = self.build_change_pubkey(command, None).await?;

        let provider = self.wallet.provider.clone();
        self.submit(command.modifier, || async {
            let tx_hash = provider.send_tx(tx, eth_signature).await?;
            Ok(SyncTransactionHandle::new(tx_hash, provider))
        })
        .await
    }

    pub(super) async fn build_change_pubkey(
        &self,
        command: &TxCommand,
        nonce: Option<Nonce>,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        let mut builder = self
            .wallet
            .start_change_pubkey()
            .fee_token(self.config.main_token.as_str())
            .unwrap();

        if let Some(nonce) = nonce {
            builder = builder.nonce(nonce);
        }

        let tx = builder.tx().await.map_err(Self::tx_creation_error)?;

        Ok(self.apply_modifier(tx, None, command.modifier))
    }

    async fn execute_transfer(&self, command: &TxCommand) -> Result<ReportLabel, ClientError> {
        let (tx, eth_signature) = self.build_transfer(command, None).await?;

        let provider = self.wallet.provider.clone();
        self.submit(command.modifier, || async {
            let tx_hash = provider.send_tx(tx, eth_signature).await?;
            Ok(SyncTransactionHandle::new(tx_hash, provider))
        })
        .await
    }

    pub(super) async fn build_transfer(
        &self,
        command: &TxCommand,
        nonce: Option<Nonce>,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        let mut builder = self
            .wallet
            .start_transfer()
            .to(command.to)
            .amount(command.amount.clone())
            .token(self.config.main_token.as_str())
            .unwrap();

        if let Some(nonce) = nonce {
            builder = builder.nonce(nonce);
        }

        let (tx, eth_signature) = builder.tx().await.map_err(Self::tx_creation_error)?;

        Ok(self.apply_modifier(tx, eth_signature, command.modifier))
    }

    async fn execute_withdraw(&self, command: &TxCommand) -> Result<ReportLabel, ClientError> {
        let (tx, eth_signature) = self.build_withdraw(command, None).await?;

        let provider = self.wallet.provider.clone();
        self.submit(command.modifier, || async {
            let tx_hash = provider.send_tx(tx, eth_signature).await?;
            Ok(SyncTransactionHandle::new(tx_hash, provider))
        })
        .await
    }

    pub(super) async fn build_withdraw(
        &self,
        command: &TxCommand,
        nonce: Option<Nonce>,
    ) -> Result<(ZkSyncTx, Option<PackedEthSignature>), ClientError> {
        let mut builder = self
            .wallet
            .start_withdraw()
            .to(command.to)
            .amount(command.amount.clone())
            .token(self.config.main_token.as_str())
            .unwrap();
        if let Some(nonce) = nonce {
            builder = builder.nonce(nonce);
        }

        let (tx, eth_signature) = builder.tx().await.map_err(Self::tx_creation_error)?;

        Ok(self.apply_modifier(tx, eth_signature, command.modifier))
    }
}
