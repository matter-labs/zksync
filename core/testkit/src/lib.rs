//use log::*;

use crate::eth_account::{parse_ether, ETHExecResult, EthereumAccount};
use crate::external_commands::{deploy_test_contracts, get_test_accounts, Contracts};
use crate::zksync_account::ZksyncAccount;
use bigdecimal::BigDecimal;
use failure::bail;
use futures::{
    channel::{mpsc, oneshot},
    executor::block_on,
    SinkExt, StreamExt,
};
use models::config_options::ConfigurationOptions;
use models::node::{
    Account, AccountId, AccountMap, Address, FranklinTx, Nonce, PriorityOp, TokenId,
};
use models::CommitRequest;
use server::mempool::ProposedBlock;
use server::state_keeper::{
    start_state_keeper, PlasmaStateInitParams, PlasmaStateKeeper, StateKeeperRequest,
};
use std::collections::HashMap;
use std::thread::JoinHandle;
use std::time::Instant;
use tokio::runtime::Runtime;
use web3::transports::Http;
use web3::Transport;

pub mod eth_account;
pub mod external_commands;
pub mod zksync_account;
use models::EncodedProof;
use web3::types::U64;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct ETHAccountId(pub usize);
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct ZKSyncAccountId(pub usize);
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Token(pub TokenId);

/// Account set is used to create transactions using stored account
/// in a convenient way
pub struct AccountSet<T: Transport> {
    pub eth_accounts: Vec<EthereumAccount<T>>,
    pub zksync_accounts: Vec<ZksyncAccount>,
    pub fee_account_id: ZKSyncAccountId,
}
impl<T: Transport> AccountSet<T> {
    /// Create deposit from eth account to zksync account
    pub fn deposit(
        &self,
        from: ETHAccountId,
        to: ZKSyncAccountId,
        token: Option<Address>, // None for ETH
        amount: BigDecimal,
    ) -> PriorityOp {
        let from = &self.eth_accounts[from.0];
        let to = &self.zksync_accounts[to.0];

        if let Some(address) = token {
            block_on(from.deposit_erc20(address, amount, &to.address))
                .expect("erc20 deposit should not fail")
        } else {
            block_on(from.deposit_eth(amount, &to.address, None))
                .expect("eth deposit should not fail")
        }
    }

    /// Create signed transfer between zksync accounts
    /// `nonce` optional nonce override
    /// `increment_nonce` - flag for `from` account nonce increment
    #[allow(clippy::too_many_arguments)]
    pub fn transfer(
        &self,
        from: ZKSyncAccountId,
        to: ZKSyncAccountId,
        token_id: Token,
        amount: BigDecimal,
        fee: BigDecimal,
        nonce: Option<Nonce>,
        increment_nonce: bool,
    ) -> FranklinTx {
        let from = &self.zksync_accounts[from.0];
        let to = &self.zksync_accounts[to.0];

        FranklinTx::Transfer(Box::new(
            from.sign_transfer(
                token_id.0,
                "",
                amount,
                fee,
                &to.address,
                nonce,
                increment_nonce,
            )
            .0,
        ))
    }

    /// Create withdraw from zksync account to eth account
    /// `nonce` optional nonce override
    /// `increment_nonce` - flag for `from` account nonce increment
    #[allow(clippy::too_many_arguments)]
    fn withdraw(
        &self,
        from: ZKSyncAccountId,
        to: ETHAccountId,
        token_id: Token,
        amount: BigDecimal,
        fee: BigDecimal,
        nonce: Option<Nonce>,
        increment_nonce: bool,
    ) -> FranklinTx {
        let from = &self.zksync_accounts[from.0];
        let to = &self.eth_accounts[to.0];

        FranklinTx::Withdraw(Box::new(
            from.sign_withdraw(
                token_id.0,
                "",
                amount,
                fee,
                &to.address,
                nonce,
                increment_nonce,
            )
            .0,
        ))
    }

    /// Create full exit from zksync account to eth account
    /// `nonce` optional nonce override
    /// `increment_nonce` - flag for `from` account nonce increment
    #[allow(clippy::too_many_arguments)]
    fn full_exit(
        &self,
        post_by: ETHAccountId,
        token_address: Address,
        account_id: AccountId,
    ) -> PriorityOp {
        block_on(self.eth_accounts[post_by.0].full_exit(account_id, token_address))
            .expect("FullExit eth call failed")
    }

    fn change_pubkey_with_onchain_auth(
        &self,
        eth_account: ETHAccountId,
        zksync_signer: ZKSyncAccountId,
        nonce: Option<Nonce>,
        increment_nonce: bool,
    ) -> FranklinTx {
        let zksync_account = &self.zksync_accounts[zksync_signer.0];
        let auth_nonce = nonce.unwrap_or_else(|| zksync_account.nonce());

        let eth_account = &self.eth_accounts[eth_account.0];
        let tx_receipt =
            block_on(eth_account.auth_fact(&zksync_account.pubkey_hash.data, auth_nonce))
                .expect("Auth pubkey fail");
        assert_eq!(tx_receipt.status, Some(U64::from(1)), "Auth pubkey fail");
        FranklinTx::ChangePubKey(Box::new(zksync_account.create_change_pubkey_tx(
            nonce,
            increment_nonce,
            true,
        )))
    }

    fn change_pubkey_with_tx(
        &self,
        zksync_signer: ZKSyncAccountId,
        nonce: Option<Nonce>,
        increment_nonce: bool,
    ) -> FranklinTx {
        let zksync_account = &self.zksync_accounts[zksync_signer.0];
        FranklinTx::ChangePubKey(Box::new(zksync_account.create_change_pubkey_tx(
            nonce,
            increment_nonce,
            false,
        )))
    }
}

/// Initialize plasma state with one account - fee account.
pub fn genesis_state(fee_account_address: &Address) -> PlasmaStateInitParams {
    let mut accounts = AccountMap::default();
    let operator_account = Account::default_with_address(fee_account_address);
    accounts.insert(0, operator_account);

    PlasmaStateInitParams {
        accounts,
        last_block_number: 0,
        unprocessed_priority_op: 0,
    }
}

pub async fn state_keeper_get_account(
    mut sender: mpsc::Sender<StateKeeperRequest>,
    address: &Address,
) -> Option<(AccountId, Account)> {
    let resp = oneshot::channel();
    sender
        .send(StateKeeperRequest::GetAccount(*address, resp.0))
        .await
        .expect("sk request send");
    resp.1.await.expect("sk account resp recv")
}

pub struct StateKeeperChannels {
    requests: mpsc::Sender<StateKeeperRequest>,
    new_blocks: mpsc::Receiver<CommitRequest>,
}

// Thread join handle and stop channel sender.
pub fn spawn_state_keeper(
    fee_account: &Address,
) -> (JoinHandle<()>, oneshot::Sender<()>, StateKeeperChannels) {
    let (proposed_blocks_sender, proposed_blocks_receiver) = mpsc::channel(256);
    let (state_keeper_req_sender, state_keeper_req_receiver) = mpsc::channel(256);
    let (executed_tx_notify_sender, _executed_tx_notify_receiver) = mpsc::channel(256);

    let state_keeper = PlasmaStateKeeper::new(
        genesis_state(fee_account),
        *fee_account,
        state_keeper_req_receiver,
        proposed_blocks_sender,
        executed_tx_notify_sender,
    );

    let (stop_state_keeper_sender, stop_state_keeper_receiver) = oneshot::channel::<()>();
    let sk_thread_handle = std::thread::spawn(move || {
        let mut main_runtime = Runtime::new().expect("main runtime start");
        start_state_keeper(state_keeper, &main_runtime);
        main_runtime.block_on(async move {
            stop_state_keeper_receiver
                .await
                .expect("stop sk sender dropped");
        })
    });

    (
        sk_thread_handle,
        stop_state_keeper_sender,
        StateKeeperChannels {
            requests: state_keeper_req_sender,
            new_blocks: proposed_blocks_receiver,
        },
    )
}

pub fn perform_basic_operations(
    token: u16,
    test_setup: &mut TestSetup,
    deposit_amount: BigDecimal,
) {
    // test deposit to other account
    test_setup.start_block();
    test_setup.deposit(
        ETHAccountId(0),
        ZKSyncAccountId(2),
        Token(token),
        deposit_amount.clone(),
    );
    test_setup
        .execute_commit_and_verify_block()
        .expect("Block execution failed");
    println!("Deposit to other account test success, token_id: {}", token);

    // test two deposits
    test_setup.start_block();
    test_setup.deposit(
        ETHAccountId(0),
        ZKSyncAccountId(1),
        Token(token),
        deposit_amount.clone(),
    );
    test_setup.deposit(
        ETHAccountId(0),
        ZKSyncAccountId(1),
        Token(token),
        deposit_amount.clone(),
    );
    test_setup
        .execute_commit_and_verify_block()
        .expect("Block execution failed");
    println!("Deposit test success, token_id: {}", token);

    // test transfers
    test_setup.start_block();

    test_setup.change_pubkey_with_onchain_auth(ETHAccountId(0), ZKSyncAccountId(1));

    //transfer to self should work
    test_setup.transfer(
        ZKSyncAccountId(1),
        ZKSyncAccountId(1),
        Token(token),
        &deposit_amount / &BigDecimal::from(8),
        &deposit_amount / &BigDecimal::from(8),
    );

    //should be executed as a transfer
    test_setup.transfer(
        ZKSyncAccountId(1),
        ZKSyncAccountId(2),
        Token(token),
        &deposit_amount / &BigDecimal::from(8),
        &deposit_amount / &BigDecimal::from(8),
    );

    let nonce = test_setup.accounts.zksync_accounts[1].nonce();
    let incorrect_nonce_transfer = test_setup.accounts.transfer(
        ZKSyncAccountId(1),
        ZKSyncAccountId(0),
        Token(token),
        deposit_amount.clone(),
        BigDecimal::from(0),
        Some(nonce + 1),
        false,
    );
    test_setup.execute_incorrect_tx(incorrect_nonce_transfer);

    //should be executed as a transfer to new
    test_setup.transfer(
        ZKSyncAccountId(1),
        ZKSyncAccountId(2),
        Token(token),
        &deposit_amount / &BigDecimal::from(4),
        &deposit_amount / &BigDecimal::from(4),
    );

    test_setup.change_pubkey_with_tx(ZKSyncAccountId(2));

    test_setup.withdraw(
        ZKSyncAccountId(2),
        ETHAccountId(0),
        Token(token),
        &deposit_amount / &BigDecimal::from(4),
        &deposit_amount / &BigDecimal::from(4),
    );
    test_setup
        .execute_commit_and_verify_block()
        .expect("Block execution failed");
    println!("Transfer test success, token_id: {}", token);

    test_setup.start_block();
    test_setup.full_exit(ETHAccountId(0), ZKSyncAccountId(1), Token(token));
    test_setup
        .execute_commit_and_verify_block()
        .expect("Block execution failed");
    println!("Full exit test success, token_id: {}", token);
}

pub fn perform_basic_tests() {
    let config = ConfigurationOptions::from_env();

    let fee_account = ZksyncAccount::rand();
    let (sk_thread_handle, stop_state_keeper_sender, sk_channels) =
        spawn_state_keeper(&fee_account.address);

    let deploy_timer = Instant::now();
    println!("deploying contracts");
    let contracts = deploy_test_contracts();
    println!(
        "contracts deployed {:#?}, {} secs",
        contracts,
        deploy_timer.elapsed().as_secs()
    );

    let (_el, transport) = Http::new(&config.web3_url).expect("http transport start");
    let commit_account = EthereumAccount::new(
        config.operator_private_key,
        config.operator_eth_addr,
        transport.clone(),
        contracts.contract,
        &config,
    );

    let eth_accounts = get_test_accounts()
        .into_iter()
        .map(|test_eth_account| {
            EthereumAccount::new(
                test_eth_account.private_key,
                test_eth_account.address,
                transport.clone(),
                contracts.contract,
                &config,
            )
        })
        .collect::<Vec<_>>();

    let zksync_accounts = {
        let mut zksync_accounts = Vec::new();
        zksync_accounts.push(fee_account);
        zksync_accounts.extend(eth_accounts.iter().map(|eth_account| {
            let rng_zksync_key = ZksyncAccount::rand().private_key;
            ZksyncAccount::new(
                rng_zksync_key,
                0,
                eth_account.address,
                eth_account.private_key,
            )
        }));
        zksync_accounts
    };

    let accounts = AccountSet {
        eth_accounts,
        zksync_accounts,
        fee_account_id: ZKSyncAccountId(0),
    };

    let mut test_setup = TestSetup::new(sk_channels, accounts, &contracts, commit_account);

    let deposit_amount = parse_ether("1.0").unwrap();

    for token in 0..=1 {
        perform_basic_operations(token, &mut test_setup, deposit_amount.clone());
    }

    stop_state_keeper_sender.send(()).expect("sk stop send");
    sk_thread_handle.join().expect("sk thread join");
}

// Struct used to keep expected balance changes after transactions execution.
#[derive(Default)]
pub struct ExpectedAccountState {
    // First number is balance, second one is allowed error in balance(used for ETH because eth is used for transaction fees).
    eth_accounts_state: HashMap<(ETHAccountId, TokenId), (BigDecimal, BigDecimal)>,
    sync_accounts_state: HashMap<(ZKSyncAccountId, TokenId), BigDecimal>,
}

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
}

impl TestSetup {
    pub fn new(
        sk_channels: StateKeeperChannels,
        accounts: AccountSet<Http>,
        deployed_contracts: &Contracts,
        commit_account: EthereumAccount<Http>,
    ) -> Self {
        let mut tokens = HashMap::new();
        tokens.insert(1, deployed_contracts.test_erc20_address.clone());
        tokens.insert(0, Address::default());
        Self {
            state_keeper_request_sender: sk_channels.requests,
            proposed_blocks_receiver: sk_channels.new_blocks,
            accounts,
            tokens,
            expected_changes_for_current_block: ExpectedAccountState::default(),
            commit_account,
        }
    }

    pub fn get_expected_eth_account_balance(
        &self,
        account: ETHAccountId,
        token: TokenId,
    ) -> (BigDecimal, BigDecimal) {
        self.expected_changes_for_current_block
            .eth_accounts_state
            .get(&(account, token))
            .cloned()
            .unwrap_or_else(|| (self.get_eth_balance(account, token), BigDecimal::from(0)))
    }

    pub fn get_expected_zksync_account_balance(
        &self,
        account: ZKSyncAccountId,
        token: TokenId,
    ) -> BigDecimal {
        self.expected_changes_for_current_block
            .sync_accounts_state
            .get(&(account, token))
            .cloned()
            .unwrap_or_else(|| self.get_zksync_balance(account, token))
    }

    pub fn start_block(&mut self) {
        self.expected_changes_for_current_block = ExpectedAccountState::default();
    }

    pub fn execute_incorrect_tx(&mut self, tx: FranklinTx) {
        self.execute_tx(tx);
    }

    pub fn deposit(
        &mut self,
        from: ETHAccountId,
        to: ZKSyncAccountId,
        token: Token,
        amount: BigDecimal,
    ) {
        let mut from_eth_balance = self.get_expected_eth_account_balance(from, token.0);
        from_eth_balance.0 -= &amount;

        self.expected_changes_for_current_block
            .eth_accounts_state
            .insert((from, token.0), from_eth_balance);

        if let Some(mut eth_balance) = self
            .expected_changes_for_current_block
            .eth_accounts_state
            .remove(&(from, 0))
        {
            eth_balance.1 += parse_ether("0.015").unwrap(); // max fee payed;
            self.expected_changes_for_current_block
                .eth_accounts_state
                .insert((from, 0), eth_balance);
        }

        let mut zksync0_old = self.get_expected_zksync_account_balance(to, token.0);
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
        let deposit = self.accounts.deposit(from, to, token_address, amount);

        self.execute_priority_op(deposit);
    }

    fn execute_tx(&self, tx: FranklinTx) {
        let block = ProposedBlock {
            priority_ops: Vec::new(),
            txs: vec![tx],
        };
        let block_sender = async {
            self.state_keeper_request_sender
                .clone()
                .send(StateKeeperRequest::ExecuteMiniBlock(block))
                .await
                .expect("sk receiver dropped");
        };
        block_on(block_sender);
    }

    fn execute_priority_op(&self, op: PriorityOp) {
        let block = ProposedBlock {
            priority_ops: vec![op],
            txs: Vec::new(),
        };
        let block_sender = async {
            self.state_keeper_request_sender
                .clone()
                .send(StateKeeperRequest::ExecuteMiniBlock(block))
                .await
                .expect("sk receiver dropped");
        };
        block_on(block_sender);
    }

    pub fn exit(
        &mut self,
        sending_account: ETHAccountId,
        token_id: Token,
        amount: &BigDecimal,
        proof: EncodedProof,
    ) -> ETHExecResult {
        block_on(self.accounts.eth_accounts[sending_account.0].exit(token_id.0, amount, proof))
            .expect("Failed to post exit tx")
    }

    pub fn full_exit(&mut self, post_by: ETHAccountId, from: ZKSyncAccountId, token: Token) {
        let account_id = self
            .get_zksync_account_committed_state(from)
            .map(|(id, _)| id)
            .expect("Account should be in the map");
        let token_address = if token.0 == 0 {
            Address::zero()
        } else {
            *self.tokens.get(&token.0).expect("Token does not exist")
        };

        let zksync0_old = self.get_expected_zksync_account_balance(from, token.0);
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((from, token.0), BigDecimal::from(0));

        let mut post_by_eth_balance = self.get_expected_eth_account_balance(post_by, token.0);
        post_by_eth_balance.0 += zksync0_old;
        self.expected_changes_for_current_block
            .eth_accounts_state
            .insert((post_by, token.0), post_by_eth_balance);

        if let Some(mut eth_balance) = self
            .expected_changes_for_current_block
            .eth_accounts_state
            .remove(&(post_by, 0))
        {
            eth_balance.1 += parse_ether("0.015").unwrap(); // max fee payed;
            self.expected_changes_for_current_block
                .eth_accounts_state
                .insert((post_by, 0), eth_balance);
        }

        let full_exit = self.accounts.full_exit(post_by, token_address, account_id);
        self.execute_priority_op(full_exit);
    }

    pub fn change_pubkey_with_tx(&mut self, zksync_signer: ZKSyncAccountId) {
        let tx = self
            .accounts
            .change_pubkey_with_tx(zksync_signer, None, true);

        self.execute_tx(tx);
    }

    pub fn change_pubkey_with_onchain_auth(
        &mut self,
        eth_account: ETHAccountId,
        zksync_signer: ZKSyncAccountId,
    ) {
        let tx =
            self.accounts
                .change_pubkey_with_onchain_auth(eth_account, zksync_signer, None, true);

        self.execute_tx(tx);
    }

    pub fn transfer(
        &mut self,
        from: ZKSyncAccountId,
        to: ZKSyncAccountId,
        token: Token,
        amount: BigDecimal,
        fee: BigDecimal,
    ) {
        let mut zksync0_old = self.get_expected_zksync_account_balance(from, token.0);
        zksync0_old -= &amount;
        zksync0_old -= &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((from, token.0), zksync0_old);

        let mut zksync0_old = self.get_expected_zksync_account_balance(to, token.0);
        zksync0_old += &amount;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((to, token.0), zksync0_old);

        let mut zksync0_old =
            self.get_expected_zksync_account_balance(self.accounts.fee_account_id, token.0);
        zksync0_old += &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((self.accounts.fee_account_id, token.0), zksync0_old);

        let transfer = self
            .accounts
            .transfer(from, to, token, amount, fee, None, true);

        self.execute_tx(transfer)
    }

    pub fn withdraw(
        &mut self,
        from: ZKSyncAccountId,
        to: ETHAccountId,
        token: Token,
        amount: BigDecimal,
        fee: BigDecimal,
    ) {
        let mut zksync0_old = self.get_expected_zksync_account_balance(from, token.0);
        zksync0_old -= &amount;
        zksync0_old -= &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((from, token.0), zksync0_old);

        let mut to_eth_balance = self.get_expected_eth_account_balance(to, token.0);
        to_eth_balance.0 += &amount;
        self.expected_changes_for_current_block
            .eth_accounts_state
            .insert((to, token.0), to_eth_balance);

        let mut zksync0_old =
            self.get_expected_zksync_account_balance(self.accounts.fee_account_id, token.0);
        zksync0_old += &fee;
        self.expected_changes_for_current_block
            .sync_accounts_state
            .insert((self.accounts.fee_account_id, token.0), zksync0_old);

        let withdraw = self
            .accounts
            .withdraw(from, to, token, amount, fee, None, true);

        self.execute_tx(withdraw);
    }

    /// Should not be used execept special cases(when we want to commit but don't want to verify block)
    pub fn execute_commit_block(&mut self) -> ETHExecResult {
        let block_sender = async {
            self.state_keeper_request_sender
                .clone()
                .send(StateKeeperRequest::SealBlock)
                .await
                .expect("sk receiver dropped");
        };
        block_on(block_sender);
        let new_block =
            block_on(self.proposed_blocks_receiver.next()).expect("State keeper channel closed");

        block_on(self.commit_account.commit_block(&new_block.block)).expect("block commit fail")
    }

    pub fn execute_commit_and_verify_block(&mut self) -> Result<(), failure::Error> {
        let block_sender = async {
            self.state_keeper_request_sender
                .clone()
                .send(StateKeeperRequest::SealBlock)
                .await
                .expect("sk receiver dropped");
        };
        block_on(block_sender);
        let new_block =
            block_on(self.proposed_blocks_receiver.next()).expect("State keeper channel closed");

        block_on(self.commit_account.commit_block(&new_block.block))
            .expect("block commit send tx")
            .expect_success();
        block_on(self.commit_account.verify_block(&new_block.block))
            .expect("block verify send tx")
            .expect_success();
        block_on(self.commit_account.complete_withdrawals())
            .expect("complete withdrawal send tx")
            .expect_success();

        let mut block_checks_failed = false;
        for ((eth_account, token), (balance, allowed_margin)) in
            &self.expected_changes_for_current_block.eth_accounts_state
        {
            let real_balance = self.get_eth_balance(*eth_account, *token);
            let diff = balance - &real_balance;
            let is_diff_valid = diff >= BigDecimal::from(0) && diff <= *allowed_margin;
            if !is_diff_valid {
                println!(
                    "eth acc: {}, token: {}, diff: {}, within bounds: {}",
                    eth_account.0, token, diff, is_diff_valid
                );
                println!("expected: {}", balance);
                println!("real: {}", real_balance);
                block_checks_failed = true;
            }
        }

        for ((zksync_account, token), balance) in
            &self.expected_changes_for_current_block.sync_accounts_state
        {
            let real = self.get_zksync_balance(*zksync_account, *token);
            let is_diff_valid = real.clone() - balance == BigDecimal::from(0);
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

        Ok(())
    }

    fn get_zksync_account_committed_state(
        &self,
        zksync_id: ZKSyncAccountId,
    ) -> Option<(AccountId, Account)> {
        let address = &self.accounts.zksync_accounts[zksync_id.0].address;
        block_on(state_keeper_get_account(
            self.state_keeper_request_sender.clone(),
            address,
        ))
    }

    fn get_zksync_balance(&self, zksync_id: ZKSyncAccountId, token: TokenId) -> BigDecimal {
        self.get_zksync_account_committed_state(zksync_id)
            .map(|(_, acc)| acc.get_balance(token))
            .unwrap_or_default()
    }

    fn get_eth_balance(&self, eth_account_id: ETHAccountId, token: TokenId) -> BigDecimal {
        let account = &self.accounts.eth_accounts[eth_account_id.0];
        if token == 0 {
            block_on(account.eth_balance()).expect("Failed to get eth balance")
        } else {
            block_on(account.erc20_balance(&self.tokens[&token]))
                .expect("Failed to get erc20 balance")
        }
    }

    pub fn get_balance_to_withdraw(
        &self,
        eth_account_id: ETHAccountId,
        token: Token,
    ) -> BigDecimal {
        block_on(self.accounts.eth_accounts[eth_account_id.0].balances_to_withdraw(token.0))
            .expect("failed to query balance to withdraws")
    }

    pub fn is_exodus(&self) -> bool {
        block_on(self.commit_account.is_exodus()).expect("Exodus query")
    }

    pub fn total_blocks_committed(&self) -> Result<u64, failure::Error> {
        block_on(self.accounts.eth_accounts[0].total_blocks_committed())
    }

    pub fn eth_block_number(&self) -> u64 {
        block_on(self.commit_account.eth_block_number()).expect("Block number query")
    }

    pub fn get_tokens(&self) -> Vec<Token> {
        self.tokens.iter().map(|(id, _)| Token(*id)).collect()
    }

    pub fn trigger_exodus_if_needed(&self, eth_account: ETHAccountId) {
        block_on(self.accounts.eth_accounts[eth_account.0].trigger_exodus_if_needed())
            .expect("Trigger exodus if needed call");
    }

    pub fn cancel_outstanding_deposits(&self, eth_account: ETHAccountId) {
        const DEPOSITS_TO_CANCEL: u64 = 100;
        block_on(
            self.accounts.eth_accounts[eth_account.0]
                .cancel_outstanding_deposits_for_exodus_mode(DEPOSITS_TO_CANCEL),
        )
        .expect("Failed to cancel outstanding deposits");
    }

    pub fn get_accounts_state(&self) -> AccountMap {
        let mut account_map = AccountMap::default();
        for id in 0..self.accounts.zksync_accounts.len() {
            if let Some((id, account)) =
                self.get_zksync_account_committed_state(ZKSyncAccountId(id))
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
    ) -> (EncodedProof, BigDecimal) {
        let owner_address = self.accounts.zksync_accounts[fund_owner.0].address;
        // restore account state
        prover::exit_proof::create_exit_proof(accounts, owner_address, token.0)
            .expect("Failed to generate exit proof")
    }
}
