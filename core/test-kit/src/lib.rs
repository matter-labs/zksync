//use log::*;

use crate::deploy_contracts::{get_test_accounts, Contracts};
use crate::eth_account::{parse_ether, EthereumAccount};
use crate::zksync_account::ZksyncAccount;
use bigdecimal::BigDecimal;
use futures::{
    channel::{mpsc, oneshot},
    executor::block_on,
    SinkExt, StreamExt,
};
use models::node::{
    Account, AccountAddress, AccountId, AccountMap, FranklinTx, Nonce, PriorityOp, TokenId,
};
use models::CommitRequest;
use server::mempool::ProposedBlock;
use server::state_keeper::{
    start_state_keeper, PlasmaStateInitParams, PlasmaStateKeeper, StateKeeperRequest,
};
use server::ConfigurationOptions;
use std::collections::HashMap;
use std::thread::JoinHandle;
use std::time::Instant;
use tokio::runtime::Runtime;
use web3::transports::Http;
use web3::types::{Address, U64};
use web3::Transport;

pub mod deploy_contracts;
pub mod eth_account;
pub mod zksync_account;

struct AccountSet<T: Transport> {
    eth_accounts: Vec<EthereumAccount<T>>,
    zksync_accounts: Vec<ZksyncAccount>,
    fee_account_id: ZKSyncAccountId,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct ETHAccountId(usize);
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct ZKSyncAccountId(usize);
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Token(TokenId);

impl<T: Transport> AccountSet<T> {
    fn deposit(
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
            block_on(from.deposit_eth(amount, &to.address)).expect("eth deposit should not fail")
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn transfer(
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

        FranklinTx::Transfer(from.sign_transfer(
            token_id.0,
            amount,
            fee,
            &to.address,
            nonce,
            increment_nonce,
        ))
    }

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

        FranklinTx::Withdraw(from.sign_withdraw(
            token_id.0,
            amount,
            fee,
            &to.address,
            nonce,
            increment_nonce,
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn full_exit(
        &self,
        post_by: ETHAccountId,
        from: ZKSyncAccountId,
        token: TokenId,
        token_address: Address,
        account_id: AccountId,
        nonce: Option<Nonce>,
        increment_nonce: bool,
    ) -> PriorityOp {
        let eth_address = self.eth_accounts[post_by.0].address;
        let signed_full_exit = self.zksync_accounts[from.0].sign_full_exit(
            account_id,
            eth_address,
            token,
            nonce,
            increment_nonce,
        );

        let mut sign = Vec::new();
        sign.extend_from_slice(signed_full_exit.signature_r.as_ref());
        sign.extend_from_slice(signed_full_exit.signature_s.as_ref());
        block_on(self.eth_accounts[post_by.0].full_exit(
            account_id,
            signed_full_exit.packed_pubkey.as_ref(),
            token_address,
            &sign,
            signed_full_exit.nonce,
        ))
        .expect("FullExit eth call failed")
    }
}

fn genesis_state(fee_account_address: &AccountAddress) -> PlasmaStateInitParams {
    let mut accounts = AccountMap::default();
    let operator_account = Account::default_with_address(fee_account_address);
    accounts.insert(0, operator_account);

    PlasmaStateInitParams {
        accounts,
        last_block_number: 0,
        unprocessed_priority_op: 0,
    }
}

async fn sk_get_account(
    mut sender: mpsc::Sender<StateKeeperRequest>,
    address: &AccountAddress,
) -> Option<(AccountId, Account)> {
    let resp = oneshot::channel();
    sender
        .send(StateKeeperRequest::GetAccount(address.clone(), resp.0))
        .await
        .expect("sk request send");
    resp.1.await.expect("sk account resp recv")
}

struct StateKeeperChannels {
    requests: mpsc::Sender<StateKeeperRequest>,
    new_blocks: mpsc::Receiver<CommitRequest>,
}

// Thread join handle and stop channel sender.
fn spawn_state_keeper(
    fee_account: &AccountAddress,
) -> (JoinHandle<()>, oneshot::Sender<()>, StateKeeperChannels) {
    let (proposed_blocks_sender, proposed_blocks_receiver) = mpsc::channel(256);
    let (state_keeper_req_sender, state_keeper_req_receiver) = mpsc::channel(256);
    let (executed_tx_notify_sender, _executed_tx_notify_receiver) = mpsc::channel(256);

    let state_keeper = PlasmaStateKeeper::new(
        genesis_state(fee_account),
        fee_account.clone(),
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

pub fn init_and_run_state_keeper() {
    let config = ConfigurationOptions::from_env();

    let fee_account = ZksyncAccount::rand();
    let (sk_thread_handle, stop_state_keeper_sender, sk_channels) =
        spawn_state_keeper(&fee_account.address);

    let deploy_timer = Instant::now();
    println!("deploying contracts");
    let contracts = deploy_contracts::deploy_contracts();
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
        .collect();

    let accounts = AccountSet {
        eth_accounts,
        zksync_accounts: vec![fee_account, ZksyncAccount::rand(), ZksyncAccount::rand()],
        fee_account_id: ZKSyncAccountId(0),
    };

    let mut test_setup = TestSetup::new(sk_channels, accounts, &contracts, commit_account);

    let deposit_amount = parse_ether("1.0").unwrap();

    // test two deposits
    test_setup.start_block();
    test_setup.deposit(
        ETHAccountId(0),
        ZKSyncAccountId(1),
        Token(0),
        deposit_amount.clone(),
    );
    test_setup.deposit(
        ETHAccountId(0),
        ZKSyncAccountId(1),
        Token(0),
        deposit_amount.clone(),
    );
    test_setup.execute_commit_and_verify_block();

    // test transfers
    test_setup.start_block();

    //should be executed as a transfer
    test_setup.transfer(
        ZKSyncAccountId(1),
        ZKSyncAccountId(2),
        Token(0),
        &deposit_amount / &BigDecimal::from(4),
        &deposit_amount / &BigDecimal::from(4),
    );

    let nonce = test_setup.accounts.zksync_accounts[1].nonce();
    let incorrect_nonce_transfer = test_setup.accounts.transfer(
        ZKSyncAccountId(1),
        ZKSyncAccountId(0),
        Token(0),
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
        Token(0),
        &deposit_amount / &BigDecimal::from(4),
        &deposit_amount / &BigDecimal::from(4),
    );

    test_setup.withdraw(
        ZKSyncAccountId(2),
        ETHAccountId(0),
        Token(0),
        &deposit_amount / &BigDecimal::from(4),
        &deposit_amount / &BigDecimal::from(4),
    );
    test_setup.execute_commit_and_verify_block();

    stop_state_keeper_sender.send(()).expect("sk stop send");
    sk_thread_handle.join().expect("sk thread join");
}

#[derive(Default)]
struct ExpectedAccountState {
    // First number is balance, second one is allowed error in balance(used for ETH because eth is used for transaction fees).
    eth_accounts_state: HashMap<(ETHAccountId, TokenId), (BigDecimal, BigDecimal)>,
    sync_accounts_state: HashMap<(ZKSyncAccountId, TokenId), BigDecimal>,
}

#[derive(Default)]
struct TestBlock {
    txs: ProposedBlock,
    expected_state: ExpectedAccountState,
}

pub struct TestSetup {
    state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
    proposed_blocks_receiver: mpsc::Receiver<CommitRequest>,

    accounts: AccountSet<Http>,
    tokens: HashMap<TokenId, Address>,

    block: TestBlock,

    commit_account: EthereumAccount<Http>,
}

impl TestSetup {
    fn new(
        sk_channels: StateKeeperChannels,
        accounts: AccountSet<Http>,
        deployed_contracts: &Contracts,
        commit_account: EthereumAccount<Http>,
    ) -> Self {
        let mut tokens = HashMap::new();
        tokens.insert(1, deployed_contracts.test_erc20_address.clone());
        Self {
            state_keeper_request_sender: sk_channels.requests,
            proposed_blocks_receiver: sk_channels.new_blocks,
            accounts,
            tokens,
            block: TestBlock::default(),
            commit_account,
        }
    }

    fn get_expected_eth_account_balance(
        &self,
        account: ETHAccountId,
        token: TokenId,
    ) -> (BigDecimal, BigDecimal) {
        self.block
            .expected_state
            .eth_accounts_state
            .get(&(account, token))
            .cloned()
            .unwrap_or_else(|| (self.get_eth_balance(account, token), BigDecimal::from(0)))
    }

    fn get_expected_zksync_account_balance(
        &self,
        account: ZKSyncAccountId,
        token: TokenId,
    ) -> BigDecimal {
        self.block
            .expected_state
            .sync_accounts_state
            .get(&(account, token))
            .cloned()
            .unwrap_or_else(|| self.get_zksync_balance(account, token))
    }

    fn start_block(&mut self) {
        self.block = TestBlock::default();
    }

    pub fn execute_incorrect_tx(&mut self, tx: FranklinTx) {
        self.block.txs.txs.push(tx);
        self.execute_current_txs();
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

        self.block
            .expected_state
            .eth_accounts_state
            .insert((from, token.0), from_eth_balance);

        if let Some(mut eth_balance) = self
            .block
            .expected_state
            .eth_accounts_state
            .remove(&(from, 0))
        {
            eth_balance.1 += parse_ether("0.015").unwrap(); // max fee payed;
            self.block
                .expected_state
                .eth_accounts_state
                .insert((from, 0), eth_balance);
        }

        let mut zksync0_old = self.get_expected_zksync_account_balance(to, token.0);
        zksync0_old += &amount;
        self.block
            .expected_state
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

        self.block.txs.priority_ops.push(deposit);
        self.execute_current_txs();
    }

    pub fn execute_current_txs(&mut self) {
        let block_sender = async {
            self.state_keeper_request_sender
                .clone()
                .send(StateKeeperRequest::ExecuteMiniBlock(self.block.txs.clone()))
                .await
                .expect("sk receiver dropped");
        };
        block_on(block_sender);
        self.block.txs = ProposedBlock::default();
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
        self.block
            .expected_state
            .sync_accounts_state
            .insert((from, token.0), BigDecimal::from(0));

        let mut post_by_eth_balance = self.get_expected_eth_account_balance(post_by, token.0);
        post_by_eth_balance.0 += zksync0_old;
        self.block
            .expected_state
            .eth_accounts_state
            .insert((post_by, token.0), post_by_eth_balance);

        if let Some(mut eth_balance) = self
            .block
            .expected_state
            .eth_accounts_state
            .remove(&(post_by, 0))
        {
            eth_balance.1 += parse_ether("0.015").unwrap(); // max fee payed;
            self.block
                .expected_state
                .eth_accounts_state
                .insert((post_by, 0), eth_balance);
        }

        let op = self.accounts.full_exit(
            post_by,
            from,
            token.0,
            token_address,
            account_id,
            None,
            true,
        );
        self.block.txs.priority_ops.push(op);
        self.execute_current_txs();
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
        self.block
            .expected_state
            .sync_accounts_state
            .insert((from, token.0), zksync0_old);

        let mut zksync0_old = self.get_expected_zksync_account_balance(to, token.0);
        zksync0_old += &amount;
        self.block
            .expected_state
            .sync_accounts_state
            .insert((to, token.0), zksync0_old);

        let mut zksync0_old =
            self.get_expected_zksync_account_balance(self.accounts.fee_account_id, token.0);
        zksync0_old += &fee;
        self.block
            .expected_state
            .sync_accounts_state
            .insert((self.accounts.fee_account_id, token.0), zksync0_old);

        let transfer1 = self
            .accounts
            .transfer(from, to, token, amount, fee, None, true);

        self.block.txs.txs.push(transfer1);
        self.execute_current_txs();
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
        self.block
            .expected_state
            .sync_accounts_state
            .insert((from, token.0), zksync0_old);

        let mut to_eth_balance = self.get_expected_eth_account_balance(to, token.0);
        to_eth_balance.0 += &amount;
        self.block
            .expected_state
            .eth_accounts_state
            .insert((to, token.0), to_eth_balance);

        let mut zksync0_old =
            self.get_expected_zksync_account_balance(self.accounts.fee_account_id, token.0);
        zksync0_old += &fee;
        self.block
            .expected_state
            .sync_accounts_state
            .insert((self.accounts.fee_account_id, token.0), zksync0_old);

        let withdraw = self
            .accounts
            .withdraw(from, to, token, amount, fee, None, true);

        self.block.txs.txs.push(withdraw);
        self.execute_current_txs();
    }

    pub fn execute_commit_and_verify_block(&mut self) {
        let block_sender = async {
            self.state_keeper_request_sender
                .clone()
                .send(StateKeeperRequest::ExecuteMiniBlock(self.block.txs.clone()))
                .await
                .expect("sk receiver dropped");
            self.state_keeper_request_sender
                .clone()
                .send(StateKeeperRequest::SealBlock)
                .await
                .expect("sk receiver dropped");
        };
        block_on(block_sender);
        let new_block = block_on(async {
            if let Some(op) = self.proposed_blocks_receiver.next().await {
                op
            } else {
                panic!("State keeper channel closed");
            }
        });

        let block_rec = block_on(self.commit_account.commit_block(&new_block.block))
            .expect("block commit fail");
        println!(
            "commit: {:?}, success: {}",
            block_rec.transaction_hash,
            block_rec.status == Some(U64::from(1))
        );
        let block_rec = block_on(self.commit_account.verify_block(&new_block.block))
            .expect("block verify fail");
        println!(
            "verify: {:?}, status: {}",
            block_rec.transaction_hash,
            block_rec.status == Some(U64::from(1))
        );

        let mut block_checks_failed = false;
        for ((eth_account, token), (balance, allowed_margin)) in
            &self.block.expected_state.eth_accounts_state
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

        for ((zksync_account, token), balance) in &self.block.expected_state.sync_accounts_state {
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
        }
    }

    fn get_zksync_account_committed_state(
        &self,
        zksync_id: ZKSyncAccountId,
    ) -> Option<(AccountId, Account)> {
        let address = &self.accounts.zksync_accounts[zksync_id.0].address;
        block_on(sk_get_account(
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
}
