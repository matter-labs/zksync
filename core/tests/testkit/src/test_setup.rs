use crate::eth_account::{get_executed_tx_fee, ETHExecResult, EthereumAccount};
use crate::external_commands::Contracts;
use anyhow::bail;
use futures::{channel::mpsc, SinkExt, StreamExt};
use num::BigUint;
use std::collections::HashMap;
use web3::transports::Http;
use zksync_core::committer::{BlockCommitRequest, CommitRequest};
use zksync_core::mempool::ProposedBlock;
use zksync_core::state_keeper::StateKeeperRequest;
use zksync_types::{
    mempool::SignedTxVariant, tx::SignedZkSyncTx, Account, AccountId, AccountMap, Address,
    PriorityOp, TokenId, ZkSyncTx,
};

use web3::types::TransactionReceipt;
use zksync_crypto::proof::EncodedProofPlonk;
use zksync_crypto::rand::Rng;
use zksync_crypto::Fr;
use zksync_types::block::Block;

use crate::account_set::AccountSet;
use crate::state_keeper_utils::*;
use crate::types::*;

/// Used to create transactions between accounts and check for their validity.
/// Every new block should start with `.start_block()`
/// and end with `execute_commit_and_verify_block()`
/// with desired transactions in between.
///
/// Transactions balance side effects are checked,
/// in order to execute unusual/failed transactions one should create it separately and commit to block
/// using `execute_incorrect_tx`
pub struct TestSetup {
    pub state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
    pub proposed_blocks_receiver: mpsc::Receiver<CommitRequest>,

    pub accounts: AccountSet<Http>,
    pub tokens: HashMap<TokenId, Address>,

    pub expected_changes_for_current_block: ExpectedAccountState,

    pub commit_account: EthereumAccount<Http>,
    pub current_state_root: Option<Fr>,
}

impl TestSetup {
    pub fn new(
        sk_channels: StateKeeperChannels,
        accounts: AccountSet<Http>,
        deployed_contracts: &Contracts,
        commit_account: EthereumAccount<Http>,
    ) -> Self {
        let mut tokens = HashMap::new();
        tokens.insert(1, deployed_contracts.test_erc20_address);
        tokens.insert(0, Address::default());
        Self {
            state_keeper_request_sender: sk_channels.requests,
            proposed_blocks_receiver: sk_channels.new_blocks,
            accounts,
            tokens,
            expected_changes_for_current_block: ExpectedAccountState::default(),
            commit_account,
            current_state_root: None,
        }
    }

    pub async fn get_expected_eth_account_balance(
        &self,
        account: ETHAccountId,
        token: TokenId,
    ) -> BigUint {
        match self
            .expected_changes_for_current_block
            .eth_accounts_state
            .get(&(account, token))
            .cloned()
        {
            Some(balance) => balance,
            None => self.get_eth_balance(account, token).await,
        }
    }

    pub async fn get_expected_zksync_account_balance(
        &self,
        account: ZKSyncAccountId,
        token: TokenId,
    ) -> BigUint {
        match self
            .expected_changes_for_current_block
            .sync_accounts_state
            .get(&(account, token))
            .cloned()
        {
            Some(balance) => balance,
            None => self.get_zksync_balance(account, token).await,
        }
    }

    pub fn start_block(&mut self) {
        self.expected_changes_for_current_block = ExpectedAccountState::default();
    }

    pub async fn execute_incorrect_tx(&mut self, tx: ZkSyncTx) {
        self.execute_tx(tx).await;
    }

    pub async fn deposit(
        &mut self,
        from: ETHAccountId,
        to: ZKSyncAccountId,
        token: Token,
        amount: BigUint,
    ) -> Vec<TransactionReceipt> {
        let mut from_eth_balance = self.get_expected_eth_account_balance(from, token.0).await;
        from_eth_balance -= &amount;

        self.expected_changes_for_current_block
            .eth_accounts_state
            .insert((from, token.0), from_eth_balance);

        let mut zksync0_old = self.get_expected_zksync_account_balance(to, token.0).await;
        zksync0_old += &amount;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((to, token.0), zksync0_old);

        let token_address = if token.0 == 0 {
            None
        } else {
            Some(
                self.tokens
                    .get(&token.0)
                    .cloned()
                    .expect("Token with token id does not exist"),
            )
        };
        let mut eth_balance = self.get_expected_eth_account_balance(from, 0).await;

        let (receipts, deposit_op) = self.accounts.deposit(from, to, token_address, amount).await;

        let mut gas_fee = BigUint::from(0u32);

        for r in &receipts {
            let current_fee =
                get_executed_tx_fee(self.commit_account.main_contract_eth_client.web3.eth(), &r)
                    .await
                    .expect("Failed to get transaction fee");

            gas_fee += current_fee;
        }

        eth_balance -= gas_fee;
        self.expected_changes_for_current_block
            .eth_accounts_state
            .insert((from, 0), eth_balance);

        self.execute_priority_op(deposit_op).await;
        receipts
    }

    async fn execute_tx(&mut self, tx: ZkSyncTx) {
        let block = ProposedBlock {
            priority_ops: Vec::new(),
            txs: vec![SignedTxVariant::from(SignedZkSyncTx::from(tx))],
        };

        // Request miniblock execution.
        self.state_keeper_request_sender
            .clone()
            .send(StateKeeperRequest::ExecuteMiniBlock(block))
            .await
            .expect("sk receiver dropped");

        // Receive the pending block processing request from state keeper.
        self.await_for_pending_block_request().await;
    }

    pub async fn deposit_to_random(
        &mut self,
        from: ETHAccountId,
        token: Token,
        amount: BigUint,
        rng: &mut impl Rng,
    ) -> Vec<TransactionReceipt> {
        let mut from_eth_balance = self.get_expected_eth_account_balance(from, token.0).await;
        from_eth_balance -= &amount;

        self.expected_changes_for_current_block
            .eth_accounts_state
            .insert((from, token.0), from_eth_balance);

        let token_address = if token.0 == 0 {
            None
        } else {
            Some(
                self.tokens
                    .get(&token.0)
                    .cloned()
                    .expect("Token with token id does not exist"),
            )
        };
        let mut eth_balance = self.get_expected_eth_account_balance(from, 0).await;

        let (receipts, deposit_op) = self
            .accounts
            .deposit_to_random(from, token_address, amount, rng)
            .await;

        let mut gas_fee = BigUint::from(0u32);

        for r in &receipts {
            let current_fee =
                get_executed_tx_fee(self.commit_account.main_contract_eth_client.web3.eth(), &r)
                    .await
                    .expect("Failed to get transaction fee");

            gas_fee += current_fee;
        }

        eth_balance -= gas_fee;
        self.expected_changes_for_current_block
            .eth_accounts_state
            .insert((from, 0), eth_balance);

        self.execute_priority_op(deposit_op).await;
        receipts
    }

    async fn execute_priority_op(&mut self, op: PriorityOp) {
        let block = ProposedBlock {
            priority_ops: vec![op],
            txs: Vec::new(),
        };

        // Request miniblock execution.
        self.state_keeper_request_sender
            .clone()
            .send(StateKeeperRequest::ExecuteMiniBlock(block))
            .await
            .expect("sk receiver dropped");

        // Receive the pending block processing request from state keeper.
        self.await_for_pending_block_request().await;
    }

    pub async fn exit(
        &mut self,
        sending_account: ETHAccountId,
        account_id: AccountId,
        token_id: Token,
        amount: &BigUint,
        proof: EncodedProofPlonk,
    ) -> ETHExecResult {
        self.accounts.eth_accounts[sending_account.0]
            .exit(account_id, token_id.0, amount, proof)
            .await
            .expect("Exit failed")
    }

    pub async fn full_exit(
        &mut self,
        post_by: ETHAccountId,
        from: ZKSyncAccountId,
        token: Token,
    ) -> TransactionReceipt {
        let account_id = self
            .get_zksync_account_committed_state(from)
            .await
            .map(|(id, _)| id)
            .expect("Account should be in the map");
        let token_address = if token.0 == 0 {
            Address::zero()
        } else {
            *self.tokens.get(&token.0).expect("Token does not exist")
        };

        let zksync0_old = self
            .get_expected_zksync_account_balance(from, token.0)
            .await;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((from, token.0), BigUint::from(0u32));

        let mut post_by_eth_balance = self
            .get_expected_eth_account_balance(post_by, token.0)
            .await;
        post_by_eth_balance += zksync0_old;
        self.expected_changes_for_current_block
            .eth_accounts_state
            .insert((post_by, token.0), post_by_eth_balance);

        let mut eth_balance = self.get_expected_eth_account_balance(post_by, 0).await;

        let (receipt, full_exit_op) = self
            .accounts
            .full_exit(post_by, token_address, account_id)
            .await;

        let gas_fee = get_executed_tx_fee(
            self.commit_account.main_contract_eth_client.web3.eth(),
            &receipt,
        )
        .await
        .expect("Failed to get transaction fee");
        eth_balance -= gas_fee;
        self.expected_changes_for_current_block
            .eth_accounts_state
            .insert((post_by, 0), eth_balance);

        self.execute_priority_op(full_exit_op).await;
        receipt
    }

    pub async fn change_pubkey_with_tx(
        &mut self,
        account: ZKSyncAccountId,
        fee_token: Token,
        fee: BigUint,
    ) {
        let account_id = self
            .get_zksync_account_committed_state(account)
            .await
            .expect("can't change pubkey, account does not exist")
            .0;
        self.accounts.zksync_accounts[account.0].set_account_id(Some(account_id));

        // Execute transaction
        let tx = self
            .accounts
            .change_pubkey_with_tx(account, fee_token.0, fee, None, true);

        self.execute_tx(tx).await;
    }

    pub async fn change_pubkey_with_onchain_auth(
        &mut self,
        eth_account: ETHAccountId,
        account: ZKSyncAccountId,
        fee_token: Token,
        fee: BigUint,
    ) {
        // Subtract fee from the account
        let mut account_balance = self
            .get_expected_zksync_account_balance(account, fee_token.0)
            .await;
        account_balance -= &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((account, fee_token.0), account_balance);

        // Add fee to the fee collector account
        let mut fee_account = self
            .get_expected_zksync_account_balance(self.accounts.fee_account_id, fee_token.0)
            .await;
        fee_account += &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((self.accounts.fee_account_id, fee_token.0), fee_account);

        // Update account pubkey
        let account_id = self
            .get_zksync_account_committed_state(account)
            .await
            .expect("can't change pubkey, account does not exist")
            .0;
        self.accounts.zksync_accounts[account.0].set_account_id(Some(account_id));

        let tx = self
            .accounts
            .change_pubkey_with_onchain_auth(eth_account, account, fee_token.0, fee, None, true)
            .await;

        self.execute_tx(tx).await;
    }

    pub async fn transfer(
        &mut self,
        from: ZKSyncAccountId,
        to: ZKSyncAccountId,
        token: Token,
        amount: BigUint,
        fee: BigUint,
    ) {
        let mut zksync0_old = self
            .get_expected_zksync_account_balance(from, token.0)
            .await;
        zksync0_old -= &amount;
        zksync0_old -= &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((from, token.0), zksync0_old);

        let mut zksync0_old = self.get_expected_zksync_account_balance(to, token.0).await;
        zksync0_old += &amount;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((to, token.0), zksync0_old);

        let mut zksync0_old = self
            .get_expected_zksync_account_balance(self.accounts.fee_account_id, token.0)
            .await;
        zksync0_old += &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((self.accounts.fee_account_id, token.0), zksync0_old);

        let transfer = self
            .accounts
            .transfer(from, to, token, amount, fee, None, true);

        self.execute_tx(transfer).await;
    }

    pub async fn transfer_to_new_random(
        &mut self,
        from: ZKSyncAccountId,
        token: Token,
        amount: BigUint,
        fee: BigUint,
        rng: &mut impl Rng,
    ) {
        let mut zksync0_old = self
            .get_expected_zksync_account_balance(from, token.0)
            .await;
        zksync0_old -= &amount;
        zksync0_old -= &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((from, token.0), zksync0_old);

        let mut zksync0_old = self
            .get_expected_zksync_account_balance(self.accounts.fee_account_id, token.0)
            .await;
        zksync0_old += &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((self.accounts.fee_account_id, token.0), zksync0_old);

        let transfer = self
            .accounts
            .transfer_to_new_random(from, token, amount, fee, None, true, rng);

        self.execute_tx(transfer).await;
    }

    fn increase_block_withdraws_amount(&mut self) {
        self.expected_changes_for_current_block.withdraw_ops += 1;

        if self.expected_changes_for_current_block.withdraw_ops
            > crate::MAX_WITHDRAWALS_PER_BLOCK as usize
        {
            panic!(
                "Attempt to perform too many withdraw operations in one block. \
                Maximum amount of withdraw operations in one block: {}. \
                You have to commit block if it has this amount of withdraws.",
                crate::MAX_WITHDRAWALS_PER_BLOCK
            )
        }
    }

    pub async fn withdraw(
        &mut self,
        from: ZKSyncAccountId,
        to: ETHAccountId,
        token: Token,
        amount: BigUint,
        fee: BigUint,
    ) {
        self.increase_block_withdraws_amount();

        let mut zksync0_old = self
            .get_expected_zksync_account_balance(from, token.0)
            .await;
        zksync0_old -= &amount;
        zksync0_old -= &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((from, token.0), zksync0_old);

        let mut to_eth_balance = self.get_expected_eth_account_balance(to, token.0).await;
        to_eth_balance += &amount;
        self.expected_changes_for_current_block
            .eth_accounts_state
            .insert((to, token.0), to_eth_balance);

        let mut zksync0_old = self
            .get_expected_zksync_account_balance(self.accounts.fee_account_id, token.0)
            .await;
        zksync0_old += &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((self.accounts.fee_account_id, token.0), zksync0_old);

        let withdraw = self
            .accounts
            .withdraw(from, to, token, amount, fee, None, true);

        self.execute_tx(withdraw).await;
    }

    pub async fn withdraw_to_random_account(
        &mut self,
        from: ZKSyncAccountId,
        token: Token,
        amount: BigUint,
        fee: BigUint,
        rng: &mut impl Rng,
    ) {
        self.increase_block_withdraws_amount();

        let mut zksync0_old = self
            .get_expected_zksync_account_balance(from, token.0)
            .await;
        zksync0_old -= &amount;
        zksync0_old -= &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((from, token.0), zksync0_old);

        let mut zksync0_old = self
            .get_expected_zksync_account_balance(self.accounts.fee_account_id, token.0)
            .await;
        zksync0_old += &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((self.accounts.fee_account_id, token.0), zksync0_old);

        let withdraw = self
            .accounts
            .withdraw_to_random(from, token, amount, fee, None, true, rng);

        self.execute_tx(withdraw).await;
    }

    pub async fn forced_exit(
        &mut self,
        initiator: ZKSyncAccountId,
        target: ZKSyncAccountId,
        target_eth_id: ETHAccountId,
        token_id: Token,
        fee: BigUint,
    ) {
        self.increase_block_withdraws_amount();

        let mut initiator_old = self
            .get_expected_zksync_account_balance(target, token_id.0)
            .await;
        initiator_old -= &fee;

        let target_old = self
            .get_expected_zksync_account_balance(target, token_id.0)
            .await;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((target, token_id.0), 0u64.into());

        let mut target_eth_balance = self
            .get_expected_eth_account_balance(target_eth_id, token_id.0)
            .await;
        target_eth_balance += &target_old;
        self.expected_changes_for_current_block
            .eth_accounts_state
            .insert((target_eth_id, token_id.0), target_eth_balance);

        let mut fee_account_balance = self
            .get_expected_zksync_account_balance(self.accounts.fee_account_id, token_id.0)
            .await;
        fee_account_balance += &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert(
                (self.accounts.fee_account_id, token_id.0),
                fee_account_balance,
            );

        let forced_exit = self
            .accounts
            .forced_exit(initiator, target, token_id, fee, None, true);

        self.execute_tx(forced_exit).await;
    }

    /// Waits for `CommitRequest::Block` to appear on proposed blocks receiver, ignoring
    /// the pending blocks.
    async fn await_for_block_commit_request(&mut self) -> BlockCommitRequest {
        while let Some(new_block_event) = self.proposed_blocks_receiver.next().await {
            match new_block_event {
                CommitRequest::Block((new_block, _)) => {
                    return new_block;
                }
                CommitRequest::PendingBlock(_) => {
                    // Pending blocks are ignored.
                }
            }
        }
        panic!("Proposed blocks receiver dropped");
    }

    /// Takes the next `CommitRequest` from the proposed blocks receiver and expects
    /// it to be `PendingBlock`. Panics otherwise.
    async fn await_for_pending_block_request(&mut self) {
        let new_block_event = self
            .proposed_blocks_receiver
            .next()
            .await
            .expect("StateKeeper sender dropped");
        match new_block_event {
            CommitRequest::Block((new_block, _)) => {
                panic!(
                    "Expected pending block, got full block proposed. Block: {:?}",
                    new_block
                );
            }
            CommitRequest::PendingBlock(_) => {
                // Nothing to be done.
            }
        }
    }

    /// Should not be used execept special cases(when we want to commit but don't want to verify block)
    pub async fn execute_commit_block(&mut self) -> (ETHExecResult, Block) {
        self.state_keeper_request_sender
            .clone()
            .send(StateKeeperRequest::SealBlock)
            .await
            .expect("sk receiver dropped");

        let new_block = self.await_for_block_commit_request().await;
        self.current_state_root = Some(new_block.block.new_root_hash);

        (
            self.commit_account
                .commit_block(&new_block.block)
                .await
                .expect("block commit fail"),
            new_block.block,
        )
    }

    pub async fn execute_verify_block(
        &mut self,
        block: &Block,
        proof: EncodedProofPlonk,
    ) -> ETHExecResult {
        self.commit_account
            .verify_block(block, Some(proof))
            .await
            .expect("block verify fail")
    }

    pub async fn execute_commit_and_verify_block(
        &mut self,
    ) -> Result<BlockExecutionResult, anyhow::Error> {
        self.state_keeper_request_sender
            .clone()
            .send(StateKeeperRequest::SealBlock)
            .await
            .expect("sk receiver dropped");

        let new_block = self.await_for_block_commit_request().await;

        self.current_state_root = Some(new_block.block.new_root_hash);

        let commit_result = self
            .commit_account
            .commit_block(&new_block.block)
            .await
            .expect("block commit send tx")
            .expect_success();
        let verify_result = self
            .commit_account
            .verify_block(&new_block.block, None)
            .await
            .expect("block verify send tx")
            .expect_success();
        let withdrawals_result = self
            .commit_account
            .complete_withdrawals()
            .await
            .expect("complete withdrawal send tx")
            .expect_success();
        let block_chunks = new_block.block.block_chunks_size;

        let mut block_checks_failed = false;
        for ((eth_account, token), expeted_balance) in
            &self.expected_changes_for_current_block.eth_accounts_state
        {
            let real_balance = self.get_eth_balance(*eth_account, *token).await;
            if expeted_balance != &real_balance {
                println!("eth acc: {}, token: {}", eth_account.0, token);
                println!("expected: {}", expeted_balance);
                println!("real:     {}", real_balance);
                block_checks_failed = true;
            }
        }

        for ((zksync_account, token), balance) in
            &self.expected_changes_for_current_block.sync_accounts_state
        {
            let real = self.get_zksync_balance(*zksync_account, *token).await;
            let is_diff_valid = real.clone() - balance == BigUint::from(0u32);
            if !is_diff_valid {
                println!(
                    "zksync acc {} diff {}, real: {}",
                    zksync_account.0,
                    real.clone() - balance,
                    real.clone()
                );
                block_checks_failed = true;
            }
        }

        if block_checks_failed {
            println!(
                "Failed block exec_operations: {:#?}",
                new_block.block.block_transactions
            );
            bail!("Block checks failed")
        }

        for zk_id in 0..self.accounts.zksync_accounts.len() {
            self.accounts.zksync_accounts[zk_id]
                .set_account_id(self.get_zksync_account_id(ZKSyncAccountId(zk_id)).await);
        }

        Ok(BlockExecutionResult::new(
            commit_result,
            verify_result,
            withdrawals_result,
            block_chunks,
        ))
    }

    pub async fn get_zksync_account_committed_state(
        &self,
        zksync_id: ZKSyncAccountId,
    ) -> Option<(AccountId, Account)> {
        let address = &self.accounts.zksync_accounts[zksync_id.0].address;
        state_keeper_get_account(self.state_keeper_request_sender.clone(), address).await
    }

    pub async fn get_zksync_account_id(&self, zksync_id: ZKSyncAccountId) -> Option<AccountId> {
        self.get_zksync_account_committed_state(zksync_id)
            .await
            .map(|a| a.0)
    }

    async fn get_zksync_balance(&self, zksync_id: ZKSyncAccountId, token: TokenId) -> BigUint {
        self.get_zksync_account_committed_state(zksync_id)
            .await
            .map(|(_, acc)| acc.get_balance(token))
            .unwrap_or_default()
    }

    async fn get_eth_balance(&self, eth_account_id: ETHAccountId, token: TokenId) -> BigUint {
        let account = &self.accounts.eth_accounts[eth_account_id.0];
        let result = if token == 0 {
            account
                .eth_balance()
                .await
                .expect("Failed to get eth balance")
        } else {
            account
                .erc20_balance(&self.tokens[&token])
                .await
                .expect("Failed to get erc20 balance")
        };
        result
            + self
                .get_balance_to_withdraw(eth_account_id, Token(token))
                .await
    }

    pub async fn get_balance_to_withdraw(
        &self,
        eth_account_id: ETHAccountId,
        token: Token,
    ) -> BigUint {
        self.accounts.eth_accounts[eth_account_id.0]
            .balances_to_withdraw(token.0)
            .await
            .expect("failed to query balance to withdraws")
    }

    pub async fn is_exodus(&self) -> bool {
        self.commit_account.is_exodus().await.expect("Exodus query")
    }

    pub async fn total_blocks_committed(&self) -> Result<u64, anyhow::Error> {
        self.accounts.eth_accounts[0].total_blocks_committed().await
    }

    pub async fn total_blocks_verified(&self) -> Result<u64, anyhow::Error> {
        self.accounts.eth_accounts[0].total_blocks_verified().await
    }

    pub async fn revert_blocks(&self, blocks_to_revert: u64) -> Result<(), anyhow::Error> {
        self.commit_account.revert_blocks(blocks_to_revert).await?;
        Ok(())
    }

    pub async fn eth_block_number(&self) -> u64 {
        self.commit_account
            .eth_block_number()
            .await
            .expect("Block number query")
    }

    pub fn get_tokens(&self) -> Vec<Token> {
        self.tokens.iter().map(|(id, _)| Token(*id)).collect()
    }

    pub async fn trigger_exodus_if_needed(&self, eth_account: ETHAccountId) {
        self.accounts.eth_accounts[eth_account.0]
            .trigger_exodus_if_needed()
            .await
            .expect("Trigger exodus if needed call");
    }

    pub async fn cancel_outstanding_deposits(&self, eth_account: ETHAccountId) {
        const DEPOSITS_TO_CANCEL: u64 = 100;
        self.accounts.eth_accounts[eth_account.0]
            .cancel_outstanding_deposits_for_exodus_mode(DEPOSITS_TO_CANCEL)
            .await
            .expect("Failed to cancel outstanding deposits");
    }

    pub async fn get_accounts_state(&self) -> AccountMap {
        let mut account_map = AccountMap::default();
        for id in 0..self.accounts.zksync_accounts.len() {
            if let Some((id, account)) = self
                .get_zksync_account_committed_state(ZKSyncAccountId(id))
                .await
            {
                account_map.insert(id, account);
            }
        }
        account_map
    }

    pub fn gen_exit_proof(
        &self,
        accounts: AccountMap,
        fund_owner: ZKSyncAccountId,
        token: Token,
    ) -> (EncodedProofPlonk, BigUint) {
        let owner = &self.accounts.zksync_accounts[fund_owner.0];
        let owner_id = owner
            .get_account_id()
            .expect("Account should have id to exit");
        // restore account state
        zksync_prover::exit_proof::create_exit_proof(accounts, owner_id, owner.address, token.0)
            .expect("Failed to generate exit proof")
    }
}
