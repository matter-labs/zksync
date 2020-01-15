//use log::*;

use serde::{Deserialize, Serialize};

use crate::deploy_contracts::Contracts;
use crate::eth_account::{parse_ether, EthereumAccount};
use crate::zksync_account::ZksyncAccount;
use bigdecimal::BigDecimal;
use eth_client::ETHClient;
use franklin_crypto::eddsa::{PrivateKey, PublicKey, Signature};
use futures::{
    channel::{mpsc, oneshot},
    compat::Future01CompatExt,
    executor::block_on,
    SinkExt, StreamExt, TryFutureExt,
};
use models::node::tx::TxSignature;
use models::node::{
    Account, AccountAddress, AccountId, AccountMap, Engine, FranklinTx, Nonce, PriorityOp, TokenId,
    Transfer,
};
use models::CommitRequest;
use rand::{Rng, SeedableRng, XorShiftRng};
use server::mempool::ProposedBlock;
use server::state_keeper::{
    start_state_keeper, PlasmaStateInitParams, PlasmaStateKeeper, StateKeeperRequest,
};
use server::ConfigurationOptions;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::process::Command;
use std::str::FromStr;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};
use storage::ConnectionPool;
use tokio::runtime::Runtime;
use tokio::spawn;
use web3::contract::{Contract, Options};
use web3::transports::{EventLoopHandle, Http};
use web3::types::{Address, H256, U256, U64};
use web3::Transport;

pub mod deploy_contracts;
pub mod eth_account;
pub mod zksync_account;

struct AccountSet<T: Transport> {
    eth_accounts: Vec<EthereumAccount<T>>,
    zksync_accounts: Vec<ZksyncAccount>,
    fee_account_id: ZKSyncAccountSetId,
}

type ETHAccountSetId = usize;
type ZKSyncAccountSetId = usize;

enum AccountSetId {
    ETHAccount(ETHAccountSetId),
    ZKSync(ZKSyncAccountSetId),
}

impl<T: Transport> AccountSet<T> {
    fn deposit(
        &self,
        from: ETHAccountSetId,
        to: ZKSyncAccountSetId,
        token: Option<Address>, // None for ETH
        amount: BigDecimal,
    ) -> PriorityOp {
        let from = &self.eth_accounts[from];
        let to = &self.zksync_accounts[to];

        if let Some(address) = token {
            block_on(from.deposit_erc20(address, amount, &to.address))
                .expect("erc20 deposit should not fail")
        } else {
            block_on(from.deposit_eth(amount, &to.address)).expect("eth deposit should not fail")
        }
    }

    fn transfer(
        &self,
        from: ZKSyncAccountSetId,
        to: ZKSyncAccountSetId,
        token_id: TokenId,
        amount: BigDecimal,
        fee: BigDecimal,
    ) -> FranklinTx {
        let from = &self.zksync_accounts[from];
        let to = &self.zksync_accounts[to];

        FranklinTx::Transfer(from.sign_transfer(token_id, amount, fee, &to.address))
    }

    fn withdraw(
        &self,
        from: ZKSyncAccountSetId,
        to: ETHAccountSetId,
        token_id: TokenId,
        amount: BigDecimal,
        fee: BigDecimal,
    ) -> FranklinTx {
        let from = &self.zksync_accounts[from];
        let to = &self.eth_accounts[to];

        FranklinTx::Withdraw(from.sign_withdraw(token_id, amount, fee, &to.address))
    }

    fn full_exit(
        &self,
        post_by: ETHAccountSetId,
        from: ZKSyncAccountSetId,
        token: TokenId,
        token_address: Address,
        account_id: AccountId,
    ) -> PriorityOp {
        let eth_address = self.eth_accounts[post_by].address.clone();
        let signed_full_exit =
            self.zksync_accounts[from].sign_full_exit(account_id, eth_address, token);

        let mut sign = Vec::new();
        sign.extend_from_slice(signed_full_exit.signature_r.as_ref());
        sign.extend_from_slice(signed_full_exit.signature_s.as_ref());
        block_on(self.eth_accounts[post_by].full_exit(
            account_id,
            signed_full_exit.packed_pubkey.as_ref(),
            token_address,
            &sign,
            signed_full_exit.nonce,
        ))
        .expect("FullExit eth call failed")
    }
}

fn gen_pk() -> PrivateKey<Engine> {
    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);

    PrivateKey(rng.gen())
}

fn new_transfer() -> FranklinTx {
    let pk = gen_pk();
    let mut transfer = Transfer {
        from: AccountAddress::default(),
        to: AccountAddress::default(),
        token: 0,
        amount: BigDecimal::from(0),
        fee: BigDecimal::from(0),
        nonce: 0,
        signature: TxSignature::default(),
    };
    transfer.signature = TxSignature::sign_musig_pedersen(&pk, &transfer.get_bytes());
    FranklinTx::Transfer(transfer)
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

fn dummy_proposed_block() -> ProposedBlock {
    ProposedBlock {
        priority_ops: Vec::new(),
        txs: vec![new_transfer()],
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ETHAccountInfo {
    address: Address,
    private_key: H256,
}
fn get_test_accounts() -> Vec<ETHAccountInfo> {
    let result = Command::new("sh")
        .arg("print-test-accounts.sh")
        .output()
        .expect("failed to execute print test accounts script");
    if !result.status.success() {
        panic!("print test accounts script failed")
    }
    let stdout = String::from_utf8(result.stdout).expect("stdout is not valid utf8");

    for std_out_line in stdout.split_whitespace().collect::<Vec<_>>() {
        if let Ok(parsed) = serde_json::from_str(std_out_line) {
            return parsed;
        }
    }

    panic!("Print test accounts script output is not parsed correctly")
}

pub fn init_and_run_state_keeper() {
    let test_accounts = get_test_accounts();

    let connection_pool = ConnectionPool::new();
    let config = ConfigurationOptions::from_env();

    let deploy_timer = Instant::now();
    println!("deploying contracts");
    let contracts = deploy_contracts::deploy_contracts();
    println!(
        "contracts deployed {:#?}, {} secs",
        contracts,
        deploy_timer.elapsed().as_secs()
    );

    let (proposed_blocks_sender, mut proposed_blocks_receiver) = mpsc::channel(256);
    let (state_keeper_req_sender, state_keeper_req_receiver) = mpsc::channel(256);
    let (executed_tx_notify_sender, executed_tx_notify_receiver) = mpsc::channel(256);

    let fee_account = ZksyncAccount::rand();
    let state_keeper = PlasmaStateKeeper::new(
        genesis_state(&fee_account.address),
        fee_account.address.clone(),
        state_keeper_req_receiver,
        proposed_blocks_sender,
        executed_tx_notify_sender,
    );

    let (mut stop_state_keeper_sender, stop_state_keeper_receiver) = oneshot::channel::<()>();
    let sk_thread_handle = std::thread::spawn(move || {
        let mut main_runtime = Runtime::new().expect("main runtime start");
        start_state_keeper(state_keeper, &main_runtime);
        main_runtime.block_on(async move {
            stop_state_keeper_receiver.await;
        })
    });

    //    let state_proxy = StateProxy::new(&config, state_keeper_req_sender.clone());
    let (_el, transport) = Http::new(&config.web3_url).expect("http transport start");

    let commit_account = EthereumAccount::new(
        config.operator_private_key.clone(),
        config.operator_eth_addr.clone(),
        transport.clone(),
        contracts.contract.clone(),
        &config,
    );

    let eth_account = EthereumAccount::new(
        test_accounts[0].private_key,
        test_accounts[0].address,
        transport,
        contracts.contract.clone(),
        &config,
    );
    let zksync_account1 = ZksyncAccount::rand();
    let zksync_account2 = ZksyncAccount::rand();

    let accounts = AccountSet {
        eth_accounts: vec![eth_account],
        zksync_accounts: vec![fee_account, zksync_account1, zksync_account2],
        fee_account_id: 0,
    };

    let mut test_setup = TestSetup::new(
        state_keeper_req_sender.clone(),
        proposed_blocks_receiver,
        accounts,
        &contracts,
        commit_account,
    );

    let deposit_amount = parse_ether("1.0").unwrap();

    test_setup.start_block();
    test_setup.deposit(0, 1, 0, deposit_amount.clone());
    test_setup.full_exit(0, 1, 0);
    test_setup.deposit(0, 1, 0, deposit_amount.clone());
    test_setup.transfer(
        1,
        0,
        0,
        &deposit_amount / &BigDecimal::from(4),
        &deposit_amount / &BigDecimal::from(4),
    );
    test_setup.transfer(
        1,
        2,
        0,
        &deposit_amount / &BigDecimal::from(2),
        BigDecimal::from(0),
    );
    test_setup.execute_commit_and_verify_block();

    stop_state_keeper_sender.send(());

    sk_thread_handle.join().expect("sk thread join");
}

#[derive(Default)]
struct ExpectedAccountState {
    // First number is balance, second one is allowed error in balance(used for ETH because eth is used for transaction fees).
    eth_accounts_state: HashMap<(ETHAccountSetId, TokenId), (BigDecimal, BigDecimal)>,
    sync_accounts_state: HashMap<(ZKSyncAccountSetId, TokenId), BigDecimal>,
}

#[derive(Default)]
struct TestBlock {
    txs: ProposedBlock,
    expected_state: ExpectedAccountState,
}

struct TestSetup {
    state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
    proposed_blocks_receiver: mpsc::Receiver<CommitRequest>,

    accounts: AccountSet<Http>,
    tokens: HashMap<TokenId, Address>,

    block: TestBlock,

    commit_account: EthereumAccount<Http>,
}

impl TestSetup {
    fn new(
        state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
        proposed_blocks_receiver: mpsc::Receiver<CommitRequest>,
        accounts: AccountSet<Http>,
        deployed_contracts: &Contracts,
        commit_account: EthereumAccount<Http>,
    ) -> Self {
        let mut tokens = HashMap::new();
        tokens.insert(1, deployed_contracts.test_erc20_address.clone());
        Self {
            state_keeper_request_sender,
            proposed_blocks_receiver,
            accounts,
            tokens,
            block: TestBlock::default(),
            commit_account,
        }
    }

    fn get_expected_eth_account_balance(
        &self,
        account: ETHAccountSetId,
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
        account: ZKSyncAccountSetId,
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

    fn deposit(
        &mut self,
        from: ETHAccountSetId,
        to: ZKSyncAccountSetId,
        token: TokenId,
        amount: BigDecimal,
    ) {
        let mut from_eth_balance = self.get_expected_eth_account_balance(from, token);
        from_eth_balance.0 -= &amount;

        self.block
            .expected_state
            .eth_accounts_state
            .insert((from, token), from_eth_balance);

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

        let mut zksync0_old = self.get_expected_zksync_account_balance(to, token);
        zksync0_old += &amount;
        self.block
            .expected_state
            .sync_accounts_state
            .insert((to, token), zksync0_old);

        let token_address = if token == 0 {
            None
        } else {
            Some(
                self.tokens
                    .get(&token)
                    .cloned()
                    .expect("Token with token id does not exist"),
            )
        };
        let deposit = self.accounts.deposit(from, to, token_address, amount);

        self.block.txs.priority_ops.push(deposit);
        self.execute_current_txs();
    }

    fn execute_current_txs(&mut self) {
        let block_sender = async {
            self.state_keeper_request_sender
                .clone()
                .send(StateKeeperRequest::ExecuteMiniBlock(self.block.txs.clone()))
                .await;
        };
        block_on(block_sender);
        self.block.txs = ProposedBlock::default();
    }

    fn full_exit(&mut self, post_by: ETHAccountSetId, from: ZKSyncAccountSetId, token: TokenId) {
        let account_id = self
            .get_zksync_account_committed_state(from)
            .map(|(id, _)| id)
            .expect("Account should be in the map");
        let token_address = if token == 0 {
            Address::zero()
        } else {
            self.tokens
                .get(&token)
                .expect("Token does not exist")
                .clone()
        };

        let mut zksync0_old = self.get_expected_zksync_account_balance(from, token);
        self.block
            .expected_state
            .sync_accounts_state
            .insert((from, token), BigDecimal::from(0));

        let mut post_by_eth_balance = self.get_expected_eth_account_balance(post_by, token);
        post_by_eth_balance.0 += zksync0_old;
        self.block
            .expected_state
            .eth_accounts_state
            .insert((post_by, token), post_by_eth_balance);

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

        let op = self
            .accounts
            .full_exit(post_by, from, token, token_address, account_id);
        self.block.txs.priority_ops.push(op);
        self.execute_current_txs();
    }

    fn transfer(
        &mut self,
        from: ZKSyncAccountSetId,
        to: ZKSyncAccountSetId,
        token: TokenId,
        amount: BigDecimal,
        fee: BigDecimal,
    ) {
        let mut zksync0_old = self.get_expected_zksync_account_balance(from, token);
        zksync0_old -= &amount;
        zksync0_old -= &fee;
        self.block
            .expected_state
            .sync_accounts_state
            .insert((from, token), zksync0_old);

        let mut zksync0_old = self.get_expected_zksync_account_balance(to, token);
        zksync0_old += &amount;
        self.block
            .expected_state
            .sync_accounts_state
            .insert((to, token), zksync0_old);

        let mut zksync0_old =
            self.get_expected_zksync_account_balance(self.accounts.fee_account_id, token);
        zksync0_old += &fee;
        self.block
            .expected_state
            .sync_accounts_state
            .insert((self.accounts.fee_account_id, token), zksync0_old);

        let transfer1 = self.accounts.transfer(from, to, token, amount, fee);

        self.block.txs.txs.push(transfer1);
        self.execute_current_txs();
    }

    fn withdraw(
        &mut self,
        from: ZKSyncAccountSetId,
        to: ETHAccountSetId,
        token: TokenId,
        amount: BigDecimal,
        fee: BigDecimal,
    ) {
        let mut zksync0_old = self.get_expected_zksync_account_balance(from, token);
        zksync0_old -= &amount;
        zksync0_old -= &fee;
        self.block
            .expected_state
            .sync_accounts_state
            .insert((from, token), zksync0_old);

        let mut to_eth_balance = self.get_expected_eth_account_balance(to, token);
        to_eth_balance.0 += &amount;
        self.block
            .expected_state
            .eth_accounts_state
            .insert((to, token), to_eth_balance);

        let mut zksync0_old =
            self.get_expected_zksync_account_balance(self.accounts.fee_account_id, token);
        zksync0_old += &fee;
        self.block
            .expected_state
            .sync_accounts_state
            .insert((self.accounts.fee_account_id, token), zksync0_old);

        let withdraw = self.accounts.withdraw(from, to, token, amount, fee);

        self.block.txs.txs.push(withdraw);
        self.execute_current_txs();
    }

    fn execute_commit_and_verify_block(&mut self) {
        let block_sender = async {
            self.state_keeper_request_sender
                .clone()
                .send(StateKeeperRequest::ExecuteMiniBlock(self.block.txs.clone()))
                .await;
            self.state_keeper_request_sender
                .clone()
                .send(StateKeeperRequest::SealBlock)
                .await;
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

        for ((eth_account, token), (balance, allowed_margin)) in
            &self.block.expected_state.eth_accounts_state
        {
            let real_balance = self.get_eth_balance(*eth_account, *token);
            println!("expected: {}", balance);
            println!("real: {}", real_balance);
            let diff = balance - real_balance;
            let is_diff_valid = diff >= BigDecimal::from(0) && &diff <= allowed_margin;
            println!("eth acc diff: {}, within bounds: {}", diff, is_diff_valid);
        }

        for ((zksync_account, token), balance) in &self.block.expected_state.sync_accounts_state {
            let real = self.get_zksync_balance(*zksync_account, *token);
            println!(
                "zksync acc {} diff {}, real: {}",
                zksync_account,
                real.clone() - balance,
                real.clone()
            );
        }
    }

    fn get_zksync_account_committed_state(
        &self,
        zksync_id: ZKSyncAccountSetId,
    ) -> Option<(AccountId, Account)> {
        let address = &self.accounts.zksync_accounts[zksync_id].address;
        block_on(sk_get_account(
            self.state_keeper_request_sender.clone(),
            address,
        ))
    }

    fn get_zksync_balance(&self, zksync_id: ZKSyncAccountSetId, token: TokenId) -> BigDecimal {
        self.get_zksync_account_committed_state(zksync_id)
            .map(|(_, acc)| acc.get_balance(token))
            .unwrap_or_default()
    }

    fn get_eth_balance(&self, eth_account_id: ETHAccountSetId, token: TokenId) -> BigDecimal {
        let account = &self.accounts.eth_accounts[eth_account_id];
        if token == 0 {
            block_on(account.eth_balance()).expect("Failed to get eth balance")
        } else {
            block_on(account.erc20_balance(&self.tokens[&token]))
                .expect("Failed to get erc20 balance")
        }
    }
}
