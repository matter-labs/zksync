//use log::*;

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
use models::abi::FRANKLIN_CONTRACT;
use models::node::tx::TxSignature;
use models::node::{
    Account, AccountAddress, AccountMap, Engine, FranklinTx, Nonce, PriorityOp, TokenId, Transfer,
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
use std::str::FromStr;
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};
use storage::ConnectionPool;
use tokio::runtime::Runtime;
use tokio::spawn;
use web3::contract::{Contract, Options};
use web3::transports::{EventLoopHandle, Http};
use web3::types::{Address, H256, U256};
use web3::Transport;

pub mod deploy_contracts;
pub mod eth_account;
pub mod zksync_account;

struct AccountSet<T: Transport> {
    eth_accounts: Vec<EthereumAccount<T>>,
    zksync_accounts: Vec<ZksyncAccount>,
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
        token_id: TokenId,
        amount: BigDecimal,
        fee: BigDecimal,
    ) -> PriorityOp {
        let from = &self.eth_accounts[from];
        let to = &self.zksync_accounts[to];

        block_on(from.deposit_eth(amount, fee, &to.address)).expect("deposit should not fail")
    }

    fn transfer(
        &self,
        from: ZKSyncAccountSetId,
        to: ZKSyncAccountSetId,
        token_id: TokenId,
        amount: BigDecimal,
        fee: BigDecimal,
        nonce: Nonce,
    ) -> FranklinTx {
        let from = &self.zksync_accounts[from];
        let to = &self.zksync_accounts[to];

        FranklinTx::Transfer(from.sign_transfer(token_id, amount, fee, &to.address, nonce))
    }

    fn withdraw(
        &self,
        from: ZKSyncAccountSetId,
        to: ETHAccountSetId,
        token_id: TokenId,
        amount: BigDecimal,
        fee: BigDecimal,
        nonce: Nonce,
    ) -> FranklinTx {
        let from = &self.zksync_accounts[from];
        let to = &self.eth_accounts[to];

        FranklinTx::Withdraw(from.sign_withdraw(token_id, amount, fee, &to.address, nonce))
    }
}

struct TestBlock {
    transactions: Vec<FranklinTx>,
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

fn genesis_state(config_opts: &ConfigurationOptions) -> PlasmaStateInitParams {
    let mut accounts = AccountMap::default();
    let operator_account = Account::default_with_address(&config_opts.operator_franklin_addr);
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
) -> Account {
    let resp = oneshot::channel();
    sender
        .send(StateKeeperRequest::GetAccount(address.clone(), resp.0))
        .await
        .expect("sk request send");
    resp.1
        .await
        .expect("sk account resp recv")
        .unwrap_or_else(|| Account::default_with_address(address))
}

pub fn init_and_run_state_keeper() {
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

    let state_keeper = PlasmaStateKeeper::new(
        genesis_state(&config),
        config.operator_franklin_addr.clone(),
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
        config.operator_private_key,
        config.operator_eth_addr,
        transport,
        contracts.contract.clone(),
        &config,
    );
    let zksync_account1 = ZksyncAccount::rand();
    let zksync_account2 = ZksyncAccount::rand();

    let accounts = AccountSet {
        eth_accounts: vec![eth_account],
        zksync_accounts: vec![zksync_account1, zksync_account2],
    };

    //    let mut expected_balances = HashMap::new();
    let zksync_acc_balance =
        |address| block_on(sk_get_account(state_keeper_req_sender.clone(), address)).get_balance(0);

    let deposit_amount = parse_ether("1.0").unwrap();

    let eth_accounts_balances = RefCell::new(HashMap::<usize, (BigDecimal, BigDecimal)>::new());
    let zksync_accounts_balances = RefCell::new(HashMap::<usize, BigDecimal>::new());

    let get_expected_eth_account_balance = |id| -> (BigDecimal, BigDecimal) {
        eth_accounts_balances
            .borrow()
            .get(&id)
            .cloned()
            .unwrap_or_else(|| {
                (
                    block_on(accounts.eth_accounts[id].eth_balance()).expect("eth balance get"),
                    BigDecimal::from(0),
                )
            })
    };
    let get_expected_zksync_account_balance = |id| -> BigDecimal {
        zksync_accounts_balances
            .borrow()
            .get(&id)
            .cloned()
            .unwrap_or_else(|| zksync_acc_balance(&accounts.zksync_accounts[id].address))
    };

    // approximate because eth is also spent on the tx fee
    let mut eth0_old = get_expected_eth_account_balance(0);
    eth0_old.0 -= &deposit_amount;
    eth0_old.1 += parse_ether("0.015").unwrap(); // max fee payed
    eth_accounts_balances.borrow_mut().insert(0, eth0_old);
    let mut zksync0_old = get_expected_zksync_account_balance(0);
    zksync0_old += &deposit_amount;
    zksync_accounts_balances.borrow_mut().insert(0, zksync0_old);

    let deposit = accounts.deposit(0, 0, 0, deposit_amount, parse_ether("0.0").unwrap());

    let transfer_amount = parse_ether("0.25").unwrap();
    let mut zksync0_old = get_expected_zksync_account_balance(0);
    zksync0_old -= &transfer_amount;
    zksync_accounts_balances.borrow_mut().insert(0, zksync0_old);
    let mut zksync1_old = get_expected_zksync_account_balance(1);
    zksync1_old += &transfer_amount;
    zksync_accounts_balances.borrow_mut().insert(1, zksync1_old);
    let transfer1 = accounts.transfer(0, 1, 0, transfer_amount, BigDecimal::from(0), 0);

    let withdraw_amount = parse_ether("0.5").unwrap();

    let mut zksync0_old = get_expected_zksync_account_balance(0);
    zksync0_old -= &withdraw_amount;
    zksync_accounts_balances.borrow_mut().insert(0, zksync0_old);
    let mut eth0_old = get_expected_eth_account_balance(0);
    eth0_old.0 += &withdraw_amount;
    eth_accounts_balances.borrow_mut().insert(0, eth0_old);

    let withdraw = accounts.withdraw(0, 0, 0, withdraw_amount, BigDecimal::from(0), 1);

    let block = ProposedBlock {
        priority_ops: vec![deposit],
        txs: vec![transfer1, withdraw],
    };

    let empty_block = async {
        state_keeper_req_sender
            .clone()
            .send(StateKeeperRequest::ExecuteMiniBlock(block))
            .await;
        state_keeper_req_sender
            .clone()
            .send(StateKeeperRequest::SealBlock)
            .await;
    };

    block_on(empty_block);

    let mut next_block = || -> CommitRequest {
        block_on(async {
            if let Some(op) = proposed_blocks_receiver.next().await {
                //                println!("op: {:#?}", op);
                return op;
            } else {
                panic!("State keeper channel closed");
            }
        })
    };

    let timer = Instant::now();
    let new_block = next_block();
    println!("block created, {}", timer.elapsed().as_secs());

    let block_rec =
        block_on(commit_account.commit_block(&new_block.block)).expect("block commit fail");
    println!(
        "commit: {:?}, status: {:?}",
        block_rec.transaction_hash, block_rec.status
    );
    let block_rec =
        block_on(commit_account.verify_block(&new_block.block)).expect("block verify fail");
    println!(
        "verify: {:?}, status: {:?}",
        block_rec.transaction_hash, block_rec.status
    );

    for (eth_acc, balance) in eth_accounts_balances.into_inner() {
        let real_balance =
            block_on(accounts.eth_accounts[eth_acc].eth_balance()).expect("eth balance get");
        let diff = balance.0 - real_balance;
        let is_diff_valid = diff > BigDecimal::from(0) && diff < balance.1;
        println!("eth acc diff: {}, within bounds: {}", diff, is_diff_valid);
    }

    for (zksync_acc, balance) in zksync_accounts_balances.into_inner() {
        let real = zksync_acc_balance(&accounts.zksync_accounts[zksync_acc].address);
        println!("zksync acc {} diff {}", zksync_acc, real - balance);
    }

    stop_state_keeper_sender.send(());

    sk_thread_handle.join().expect("sk thread join");
}

struct StateProxy {
    web3_event_loop: EventLoopHandle,
    web3_transport: Http,
    state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
    config: ConfigurationOptions,
}

impl StateProxy {
    fn new(
        config: &ConfigurationOptions,
        state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
    ) -> Self {
        let (eloop, transport) = Http::new(&config.web3_url).expect("web3 transport");
        Self {
            web3_event_loop: eloop,
            web3_transport: transport,
            state_keeper_request_sender,
            config: config.clone(),
        }
    }

    fn emergency_withdraw(&self) {
        unimplemented!()
    }

    fn get_offhcain_balance(&self, address: &AccountAddress, token: String) -> BigDecimal {
        // ask state keeper
        unimplemented!()
    }

    fn get_onchain_balance(&self, address: &Address, token: String) -> BigDecimal {
        // ask ethereum
        unimplemented!()
    }
}
