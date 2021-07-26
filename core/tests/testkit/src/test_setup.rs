use crate::eth_account::{get_executed_tx_fee, ETHExecResult, EthereumAccount};
use crate::external_commands::Contracts;
use anyhow::bail;
use futures::{
    channel::{mpsc, oneshot},
    SinkExt, StreamExt,
};
use num::{bigint::Sign, BigInt, BigUint, ToPrimitive, Zero};
use std::collections::HashMap;
use zksync_core::committer::{BlockCommitRequest, CommitRequest};
use zksync_core::mempool::ProposedBlock;
use zksync_core::state_keeper::{StateKeeperRequest, ZkSyncStateInitParams};
use zksync_types::{
    aggregated_operations::{BlocksCommitOperation, BlocksExecuteOperation, BlocksProofOperation},
    block::Block,
    mempool::SignedTxVariant,
    tx::SignedZkSyncTx,
    Account, AccountId, AccountMap, Address, BlockNumber, Fr, PriorityOp, TokenId, ZkSyncTx, H256,
    U256,
};

use web3::types::TransactionReceipt;
use zksync_crypto::proof::{EncodedAggregatedProof, EncodedSingleProof};
use zksync_crypto::rand::Rng;

use crate::account_set::AccountSet;
use crate::state_keeper_utils::*;
use crate::types::*;

use zksync_crypto::params::{NFT_STORAGE_ACCOUNT_ADDRESS, NFT_TOKEN_ID};
use zksync_types::tx::TimeRange;

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

    pub accounts: AccountSet,
    pub tokens: HashMap<TokenId, Address>,

    pub expected_changes_for_current_block: ExpectedAccountState,

    pub commit_account: EthereumAccount,
    pub current_state_root: Option<Fr>,

    pub last_committed_block: Block,
}

#[derive(Debug)]
pub struct EthAccountTransfer {
    pub account_id: ETHAccountId,
    pub token_id: TokenId,
    pub amount: BigInt,
}
#[derive(Debug)]
pub struct ZkSyncAccountTransfer {
    pub account_id: ZKSyncAccountId,
    pub token_id: TokenId,
    pub amount: BigInt,
}

#[derive(Debug)]
pub enum AccountTransfer {
    EthAccountTransfer(EthAccountTransfer),
    ZkSyncAccountTransfer(ZkSyncAccountTransfer),
}

impl TestSetup {
    pub fn new(
        sk_channels: StateKeeperChannels,
        accounts: AccountSet,
        deployed_contracts: &Contracts,
        commit_account: EthereumAccount,
        initial_root: Fr,
        last_block: Option<Block>,
    ) -> Self {
        let mut tokens = HashMap::new();
        tokens.insert(TokenId(1), deployed_contracts.test_erc20_address);
        tokens.insert(TokenId(0), Address::default());
        Self {
            state_keeper_request_sender: sk_channels.requests,
            proposed_blocks_receiver: sk_channels.new_blocks,
            accounts,
            tokens,
            expected_changes_for_current_block: ExpectedAccountState::default(),
            commit_account,
            current_state_root: None,
            last_committed_block: last_block.unwrap_or_else(|| {
                Block::new(
                    BlockNumber(0),
                    initial_root,
                    AccountId(0),
                    vec![],
                    (0, 0),
                    0,
                    U256::from(0),
                    U256::from(0),
                    H256::default(),
                    0,
                )
            }),
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
    ) -> (Vec<TransactionReceipt>, PriorityOp) {
        self.setup_basic_l1_balances(from, token).await;
        self.setup_basic_l2_balances(to, token).await;

        let (receipts, deposit_op, transfers) = self.create_deposit(from, to, token, amount).await;
        self.apply_transfers(&transfers);
        (receipts, deposit_op)
    }

    pub async fn setup_basic_l1_balances(&mut self, eth_account_id: ETHAccountId, token: Token) {
        if !self
            .expected_changes_for_current_block
            .eth_accounts_state
            .contains_key(&(eth_account_id, TokenId(0)))
        {
            // Setup eth balance for fee
            let balance = self.get_eth_balance(eth_account_id, TokenId(0)).await;
            self.expected_changes_for_current_block
                .eth_accounts_state
                .insert((eth_account_id, TokenId(0)), balance);
        }

        if !self
            .expected_changes_for_current_block
            .eth_accounts_state
            .contains_key(&(eth_account_id, token.0))
        {
            // Setup token balance
            let balance = self.get_eth_balance(eth_account_id, token.0).await;
            self.expected_changes_for_current_block
                .eth_accounts_state
                .insert((eth_account_id, token.0), balance);
        }
    }

    pub async fn setup_basic_l2_balances(&mut self, zk_account_id: ZKSyncAccountId, token: Token) {
        // Setup zksync balance
        let balance = self.get_zksync_balance(zk_account_id, token.0).await;

        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((zk_account_id, token.0), balance);
    }

    pub fn apply_transfers(&mut self, transfers: &[AccountTransfer]) {
        for transfer in transfers {
            match transfer {
                AccountTransfer::EthAccountTransfer(tr) => {
                    let key = (tr.account_id, tr.token_id);
                    let mut balance = self
                        .expected_changes_for_current_block
                        .eth_accounts_state
                        .get(&key)
                        .cloned()
                        .unwrap_or_default();
                    let (sign, amount) = tr.amount.clone().into_parts();
                    match sign {
                        Sign::Minus => {
                            balance -= amount;
                        }
                        Sign::NoSign => {
                            assert!(amount.is_zero());
                        }
                        Sign::Plus => {
                            balance += amount;
                        }
                    }
                    self.expected_changes_for_current_block
                        .eth_accounts_state
                        .insert(key, balance);
                }
                AccountTransfer::ZkSyncAccountTransfer(tr) => {
                    let key = (tr.account_id, tr.token_id);
                    let mut balance = self
                        .expected_changes_for_current_block
                        .sync_accounts_state
                        .get(&key)
                        .cloned()
                        .unwrap_or_default();
                    let (sign, amount) = tr.amount.clone().into_parts();
                    match sign {
                        Sign::Minus => {
                            balance -= amount;
                        }
                        Sign::NoSign => {
                            assert!(amount.is_zero());
                        }
                        Sign::Plus => {
                            balance += amount;
                        }
                    }
                    self.expected_changes_for_current_block
                        .sync_accounts_state
                        .insert(key, balance);
                }
            }
        }
    }

    pub async fn create_deposit(
        &mut self,
        from: ETHAccountId,
        to: ZKSyncAccountId,
        token: Token,
        amount: BigUint,
    ) -> (Vec<TransactionReceipt>, PriorityOp, Vec<AccountTransfer>) {
        let mut transfers = vec![
            AccountTransfer::EthAccountTransfer(EthAccountTransfer {
                account_id: from,
                token_id: token.0,
                amount: BigInt::from_biguint(Sign::Minus, amount.clone()),
            }),
            AccountTransfer::ZkSyncAccountTransfer(ZkSyncAccountTransfer {
                account_id: to,
                token_id: token.0,
                amount: BigInt::from_biguint(Sign::Plus, amount.clone()),
            }),
        ];

        let token_address = if token.0 == TokenId(0) {
            None
        } else {
            Some(
                self.tokens
                    .get(&token.0)
                    .cloned()
                    .expect("Token with token id does not exist"),
            )
        };

        let (receipts, deposit_op) = self.accounts.deposit(from, to, token_address, amount).await;

        let mut gas_fee = BigUint::from(0u32);

        for r in &receipts {
            let current_fee =
                get_executed_tx_fee(&self.commit_account.main_contract_eth_client, &r)
                    .await
                    .expect("Failed to get transaction fee");

            gas_fee += current_fee;
        }

        transfers.push(AccountTransfer::EthAccountTransfer(EthAccountTransfer {
            account_id: from,
            token_id: TokenId(0),
            amount: BigInt::from_biguint(Sign::Minus, gas_fee),
        }));

        self.execute_priority_op(deposit_op.clone()).await;

        (receipts, deposit_op, transfers)
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
        self.setup_basic_l1_balances(from, token).await;
        let (rec, transfers) = self
            .create_deposit_to_random(from, token, amount, rng)
            .await;
        self.apply_transfers(&transfers);
        rec
    }

    pub async fn create_deposit_to_random(
        &mut self,
        from: ETHAccountId,
        token: Token,
        amount: BigUint,
        rng: &mut impl Rng,
    ) -> (Vec<TransactionReceipt>, Vec<AccountTransfer>) {
        let mut transfers = vec![AccountTransfer::EthAccountTransfer(EthAccountTransfer {
            account_id: from,
            token_id: token.0,
            amount: BigInt::from_biguint(Sign::Minus, amount.clone()),
        })];

        let token_address = if token.0 == TokenId(0) {
            None
        } else {
            Some(
                self.tokens
                    .get(&token.0)
                    .cloned()
                    .expect("Token with token id does not exist"),
            )
        };

        let (receipts, deposit_op) = self
            .accounts
            .deposit_to_random(from, token_address, amount, rng)
            .await;

        let mut gas_fee = BigUint::from(0u32);

        for r in &receipts {
            let current_fee =
                get_executed_tx_fee(&self.commit_account.main_contract_eth_client, &r)
                    .await
                    .expect("Failed to get transaction fee");

            gas_fee += current_fee;
        }

        transfers.push(AccountTransfer::EthAccountTransfer(EthAccountTransfer {
            account_id: from,
            token_id: TokenId(0),
            amount: BigInt::from_biguint(Sign::Minus, gas_fee),
        }));

        self.execute_priority_op(deposit_op).await;
        (receipts, transfers)
    }

    pub async fn execute_priority_op(&mut self, op: PriorityOp) {
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
        zero_account_address: Address,
        proof: EncodedSingleProof,
    ) -> ETHExecResult {
        let last_block = &self.last_committed_block;
        self.accounts.eth_accounts[sending_account.0]
            .exit(
                last_block,
                account_id,
                token_id.0,
                amount,
                zero_account_address,
                proof,
            )
            .await
            .expect("Exit failed")
    }

    pub async fn full_exit(
        &mut self,
        post_by: ETHAccountId,
        from: ZKSyncAccountId,
        token: Token,
    ) -> (TransactionReceipt, PriorityOp) {
        self.setup_basic_l1_balances(post_by, token).await;
        self.setup_basic_l2_balances(from, token).await;
        let (rec, op, transfers) = self.create_full_exit(post_by, from, token).await;
        self.apply_transfers(&transfers);
        (rec, op)
    }

    pub async fn create_full_exit(
        &mut self,
        post_by: ETHAccountId,
        from: ZKSyncAccountId,
        token: Token,
    ) -> (TransactionReceipt, PriorityOp, Vec<AccountTransfer>) {
        let mut transfers = vec![];
        let account_id = self
            .get_zksync_account_committed_state(from)
            .await
            .map(|(id, _)| id)
            .expect("Account should be in the map");
        let token_address = if token.0 == TokenId(0) {
            Address::zero()
        } else {
            *self.tokens.get(&token.0).expect("Token does not exist")
        };

        let zksync0_old = self
            .get_expected_zksync_account_balance(from, token.0)
            .await;

        transfers.push(AccountTransfer::EthAccountTransfer(EthAccountTransfer {
            account_id: post_by,
            token_id: token.0,
            amount: BigInt::from_biguint(Sign::Plus, zksync0_old.clone()),
        }));

        transfers.push(AccountTransfer::ZkSyncAccountTransfer(
            ZkSyncAccountTransfer {
                account_id: from,
                token_id: token.0,
                amount: BigInt::from_biguint(Sign::Minus, zksync0_old),
            },
        ));

        let (receipt, full_exit_op) = self
            .accounts
            .full_exit(post_by, token_address, account_id)
            .await;

        let gas_fee = get_executed_tx_fee(&self.commit_account.main_contract_eth_client, &receipt)
            .await
            .expect("Failed to get transaction fee");

        transfers.push(AccountTransfer::EthAccountTransfer(EthAccountTransfer {
            account_id: post_by,
            token_id: TokenId(0),
            amount: BigInt::from_biguint(Sign::Minus, gas_fee),
        }));

        self.execute_priority_op(full_exit_op.clone()).await;
        (receipt, full_exit_op, transfers)
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
        let tx = self.accounts.change_pubkey_with_tx(
            account,
            fee_token.0,
            fee,
            None,
            true,
            Default::default(),
        );

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
            .change_pubkey_with_onchain_auth(
                eth_account,
                account,
                fee_token.0,
                fee,
                None,
                true,
                Default::default(),
            )
            .await;

        self.execute_tx(tx).await;
    }
    pub async fn mint_nft(
        &mut self,
        creator: ZKSyncAccountId,
        recipient: ZKSyncAccountId,
        fee_token: Token,
        content_hash: H256,
        fee: BigUint,
    ) {
        let mut zksync0_old = self
            .get_expected_zksync_account_balance(creator, fee_token.0)
            .await;
        zksync0_old -= &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((creator, fee_token.0), zksync0_old);

        let mut zksync0_old = self
            .get_expected_zksync_account_balance(self.accounts.fee_account_id, fee_token.0)
            .await;
        zksync0_old += &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((self.accounts.fee_account_id, fee_token.0), zksync0_old);

        let token_id = self.get_last_committed_nft_id().await;
        let mint_nft =
            self.accounts
                .mint_nft(creator, recipient, fee_token, content_hash, fee, None, true);

        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((recipient, TokenId(token_id + 1)), BigUint::from(1u32));

        self.execute_tx(mint_nft).await;
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn swap(
        &mut self,
        accounts: (ZKSyncAccountId, ZKSyncAccountId),
        recipients: (ZKSyncAccountId, ZKSyncAccountId),
        submitter: ZKSyncAccountId,
        tokens: (Token, Token, Token),
        amounts: (BigUint, BigUint),
        fee: BigUint,
        time_range: TimeRange,
    ) {
        let account_0_old = self
            .get_expected_zksync_account_balance(accounts.0, tokens.0 .0)
            .await;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((accounts.0, tokens.0 .0), account_0_old - &amounts.0);
        let account_1_old = self
            .get_expected_zksync_account_balance(accounts.1, tokens.1 .0)
            .await;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((accounts.1, tokens.1 .0), account_1_old - &amounts.1);

        let recipient_0_old = self
            .get_expected_zksync_account_balance(recipients.0, tokens.1 .0)
            .await;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((recipients.0, tokens.1 .0), recipient_0_old + &amounts.1);
        let recipient_1_old = self
            .get_expected_zksync_account_balance(recipients.1, tokens.0 .0)
            .await;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((recipients.1, tokens.0 .0), recipient_1_old + &amounts.0);

        let submitter_old = self
            .get_expected_zksync_account_balance(submitter, tokens.2 .0)
            .await;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((submitter, tokens.2 .0), submitter_old - &fee);

        let fee_account_old = self
            .get_expected_zksync_account_balance(self.accounts.fee_account_id, tokens.2 .0)
            .await;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert(
                (self.accounts.fee_account_id, tokens.2 .0),
                fee_account_old + &fee,
            );

        let swap = self.accounts.swap(
            accounts, recipients, submitter, tokens, amounts, fee, None, true, time_range,
        );

        self.execute_tx(swap).await;
    }

    pub async fn transfer(
        &mut self,
        from: ZKSyncAccountId,
        to: ZKSyncAccountId,
        token: Token,
        amount: BigUint,
        fee: BigUint,
        time_range: TimeRange,
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
            .transfer(from, to, token, amount, fee, None, time_range, true);

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

    pub async fn withdraw(
        &mut self,
        from: ZKSyncAccountId,
        to: ETHAccountId,
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

        let withdraw =
            self.accounts
                .withdraw(from, to, token, amount, fee, None, true, Default::default());

        self.execute_tx(withdraw).await;
    }

    pub async fn withdraw_nft(
        &mut self,
        from: ZKSyncAccountId,
        token: Token,
        fee_token: Token,
        fee: BigUint,
        rng: &mut impl Rng,
    ) {
        let mut zksync0_old = self
            .get_expected_zksync_account_balance(from, fee_token.0)
            .await;
        zksync0_old -= &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((from, fee_token.0), zksync0_old);
        let zksync0_old = self
            .get_expected_zksync_account_balance(from, token.0)
            .await;
        assert_eq!(zksync0_old, BigUint::from(1u32));
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((from, token.0), BigUint::zero());

        let mut zksync0_old = self
            .get_expected_zksync_account_balance(self.accounts.fee_account_id, fee_token.0)
            .await;
        zksync0_old += &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((self.accounts.fee_account_id, fee_token.0), zksync0_old);

        let withdraw = self
            .accounts
            .withdraw_nft(from, token, fee_token, fee, None, true, rng);

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

        let forced_exit = self.accounts.forced_exit(
            initiator,
            target,
            token_id,
            fee,
            None,
            true,
            Default::default(),
        );

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
    pub async fn execute_commit_block(&mut self) -> Block {
        self.state_keeper_request_sender
            .clone()
            .send(StateKeeperRequest::SealBlock)
            .await
            .expect("sk receiver dropped");

        let new_block = self.await_for_block_commit_request().await.block;
        // self.current_state_root = Some(new_block.new_root_hash);

        let block_commit_op = BlocksCommitOperation {
            last_committed_block: self.last_committed_block.clone(),
            blocks: vec![new_block.clone()],
        };
        self.commit_account
            .commit_block(&block_commit_op)
            .await
            .expect("block commit send tx")
            .expect_success();
        self.last_committed_block = new_block.clone();

        new_block
    }

    pub async fn execute_block(&mut self) -> Block {
        self.state_keeper_request_sender
            .clone()
            .send(StateKeeperRequest::SealBlock)
            .await
            .expect("sk receiver dropped");

        self.await_for_block_commit_request().await.block
    }

    pub async fn commit_blocks(&mut self, blocks: &[Block]) -> ETHExecResult {
        assert!(!blocks.is_empty());
        let block_commit_op = BlocksCommitOperation {
            last_committed_block: self.last_committed_block.clone(),
            blocks: blocks.to_vec(),
        };
        self.last_committed_block = blocks.last().unwrap().clone();
        self.commit_account
            .commit_block(&block_commit_op)
            .await
            .expect("block commit send tx")
    }

    pub async fn prove_blocks(
        &mut self,
        blocks: &[Block],
        proof: Option<EncodedAggregatedProof>,
    ) -> ETHExecResult {
        let proof = proof.unwrap_or_else(|| {
            let mut default_proof = EncodedAggregatedProof {
                individual_vk_inputs: Vec::new(),
                individual_vk_idxs: Vec::new(),
                ..Default::default()
            };
            for block in blocks {
                let commitment = U256::from_big_endian(block.block_commitment.as_bytes());
                default_proof.individual_vk_inputs.push(commitment);
                default_proof.individual_vk_idxs.push(U256::from(0));
            }
            default_proof
        });

        let block_proof_op = BlocksProofOperation {
            blocks: blocks.to_vec(),
            proof,
        };
        self.commit_account
            .verify_block(&block_proof_op)
            .await
            .expect("block verify send tx")
    }

    pub async fn execute_blocks_onchain(&mut self, blocks: &[Block]) -> ETHExecResult {
        let block_execute_op = BlocksExecuteOperation {
            blocks: blocks.to_vec(),
        };
        self.commit_account
            .execute_block(&block_execute_op)
            .await
            .expect("execute block tx")
    }

    pub async fn execute_verify_commitments(
        &mut self,
        proof: BlocksProofOperation,
    ) -> ETHExecResult {
        self.commit_account
            .verify_block(&proof)
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

        let new_block = self.await_for_block_commit_request().await.block;
        self.current_state_root = Some(new_block.new_root_hash);

        let block_commit_op = BlocksCommitOperation {
            last_committed_block: self.last_committed_block.clone(),
            blocks: vec![new_block.clone()],
        };
        let commit_result = self
            .commit_account
            .commit_block(&block_commit_op)
            .await
            .expect("block commit send tx")
            .expect_success();

        let mut proof = EncodedAggregatedProof::default();
        proof.individual_vk_inputs[0] =
            U256::from_big_endian(new_block.block_commitment.as_bytes());
        let block_proof_op = BlocksProofOperation {
            blocks: vec![new_block.clone()],
            proof,
        };
        let verify_result = self
            .commit_account
            .verify_block(&block_proof_op)
            .await
            .expect("block verify send tx")
            .expect_success();

        let block_execute_op = BlocksExecuteOperation {
            blocks: vec![new_block.clone()],
        };
        let withdrawals_result = self
            .commit_account
            .execute_block(&block_execute_op)
            .await
            .expect("execute block tx")
            .expect_success();

        self.last_committed_block = new_block.clone();

        let block_chunks = new_block.block_chunks_size;

        let mut block_checks_failed = false;
        for ((eth_account, token), expected_balance) in
            &self.expected_changes_for_current_block.eth_accounts_state
        {
            let real_balance = self.get_eth_balance(*eth_account, *token).await;
            if expected_balance != &real_balance {
                println!("eth acc: {}, token: {}", eth_account.0, token);
                println!("expected: {}", expected_balance);
                println!("real:     {}", real_balance);
                block_checks_failed = true;
            }
        }

        for ((zksync_account, token), balance) in
            &self.expected_changes_for_current_block.sync_accounts_state
        {
            let real = self.get_zksync_balance(*zksync_account, *token).await;
            if balance != &real {
                println!(
                    "zksync acc {} balance {}, real: {} token: {}",
                    zksync_account.0,
                    balance,
                    real.clone(),
                    token.0
                );
                block_checks_failed = true;
            }
        }

        if block_checks_failed {
            bail!("Block checks failed")
        }

        for zk_id in 0..self.accounts.zksync_accounts.len() {
            self.accounts.zksync_accounts[zk_id]
                .set_account_id(self.get_zksync_account_id(ZKSyncAccountId(zk_id)).await);
        }

        Ok(BlockExecutionResult::new(
            new_block,
            commit_result,
            verify_result,
            withdrawals_result,
            block_chunks,
        ))
    }

    pub async fn get_last_committed_nft_id(&self) -> u32 {
        let (_, account) = state_keeper_get_account(
            self.state_keeper_request_sender.clone(),
            &NFT_STORAGE_ACCOUNT_ADDRESS,
        )
        .await
        .unwrap();
        let balance = account.get_balance(NFT_TOKEN_ID).to_u32().unwrap();
        balance - 1
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

    pub async fn get_current_state(&mut self) -> ZkSyncStateInitParams {
        let (sender, receiver) = oneshot::channel();
        self.state_keeper_request_sender
            .send(StateKeeperRequest::GetCurrentState(sender))
            .await
            .expect("sk request send");
        receiver.await.unwrap()
    }

    async fn get_zksync_balance(&self, zksync_id: ZKSyncAccountId, token: TokenId) -> BigUint {
        let result = self
            .get_zksync_account_committed_state(zksync_id)
            .await
            .map(|(_, acc)| acc.get_balance(token))
            .unwrap_or_default();
        result
    }

    pub async fn get_eth_balance(&self, eth_account_id: ETHAccountId, token: TokenId) -> BigUint {
        let account = &self.accounts.eth_accounts[eth_account_id.0];
        let result = if token == TokenId(0) {
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

        let result = result
            + self
                .get_balance_to_withdraw(eth_account_id, self.tokens[&token])
                .await;
        result
    }

    pub async fn get_balance_to_withdraw(
        &self,
        eth_account_id: ETHAccountId,
        token: Address,
    ) -> BigUint {
        self.accounts.eth_accounts[eth_account_id.0]
            .balances_to_withdraw(token)
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

    pub async fn revert_blocks(&self, blocks: &[Block]) -> Result<(), anyhow::Error> {
        self.commit_account.revert_blocks(blocks).await?;
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

    pub async fn cancel_outstanding_deposits(
        &self,
        eth_account: ETHAccountId,
        number: u64,
        data: Vec<Vec<u8>>,
    ) {
        self.accounts.eth_accounts[eth_account.0]
            .cancel_outstanding_deposits_for_exodus_mode(number, data)
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

        // Also adding nft account
        if let Some((id, account)) = state_keeper_get_account(
            self.state_keeper_request_sender.clone(),
            &NFT_STORAGE_ACCOUNT_ADDRESS,
        )
        .await
        {
            account_map.insert(id, account);
        }

        account_map
    }

    pub fn gen_exit_proof_fungible(
        &self,
        accounts: AccountMap,
        fund_owner: ZKSyncAccountId,
        token: Token,
    ) -> (EncodedSingleProof, BigUint) {
        let owner = &self.accounts.zksync_accounts[fund_owner.0];
        let owner_id = owner
            .get_account_id()
            .expect("Account should have id to exit");
        // restore account state
        zksync_prover_utils::exit_proof::create_exit_proof_fungible(
            accounts,
            owner_id,
            owner.address,
            token.0,
        )
        .expect("Failed to generate exit proof")
    }
}
