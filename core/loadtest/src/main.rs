// Built-in
use std::fs::File;
use std::io::prelude::*;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::{env, thread};
// External
use bigdecimal::BigDecimal;
use futures::executor::block_on;
use futures::future::try_join_all;
use futures::try_join;
use jsonrpc_core::types::response::Output;
use log::{debug, info, trace};
use rand::Rng;
use serde::{Deserialize, Serialize};
use web3::transports::Http;
use web3::types::U256;
use web3::types::{H160, H256};
// Workspace
use models::config_options::ConfigurationOptions;
use models::node::tx::FranklinTx;
use models::node::tx::TxHash;
use testkit::eth_account::EthereumAccount;
use testkit::zksync_account::ZksyncAccount;

#[derive(Deserialize, Debug)]
struct AccountInfo {
    pub address: String,
    pub private_key: String,
}

#[derive(Deserialize)]
struct TestSpec {
    deposit_initial: u64,
    n_deposits: i32,
    deposit_from_amount: u64,
    deposit_to_amount: u64,
    n_transfers: i32,
    transfer_from_amount: u64,
    transfer_to_amount: u64,
    n_withdraws: i32,
    withdraw_from_amount: u64,
    withdraw_to_amount: u64,
    verify_timeout_sec: u64,
    input_accounts: Vec<AccountInfo>,
}

struct TestAccount {
    zk_acc: ZksyncAccount,
    eth_acc: EthereumAccount<Http>,
    eth_nonce: Mutex<u32>,
}

struct SentTransactions {
    op_serial_ids: Mutex<Vec<u32>>,
    tx_hashes: Mutex<Vec<TxHash>>,
}

impl SentTransactions {
    fn new() -> Self {
        Self {
            op_serial_ids: Mutex::new(Vec::new()),
            tx_hashes: Mutex::new(Vec::new()),
        }
    }

    fn add_op_id(&self, v: u32) {
        let mut vect = self.op_serial_ids.lock().unwrap();
        vect.push(v);
    }

    fn add_op_ids(&self, v: Vec<u32>) {
        let mut vect = self.op_serial_ids.lock().unwrap();
        vect.extend(v);
    }

    fn add_tx_hashes(&self, v: Vec<TxHash>) {
        let mut vect = self.tx_hashes.lock().unwrap();
        vect.extend(v)
    }
}

fn main() {
    env_logger::init();

    let config = ConfigurationOptions::from_env();
    let filepath = env::args().nth(1).expect("test spec file not given");
    let test_spec = read_test_spec(filepath);
    let (_el, transport) = Http::new(&config.web3_url).expect("http transport start");
    let test_accounts = Arc::new(construct_test_accounts(
        &test_spec.input_accounts,
        transport,
        &config,
    ));
    info!("sending transactions");
    let sent_txs = block_on(send_transactions(&test_accounts, &test_spec));
    info!("waiting for all transactions to be verified");
    block_on(wait_for_verify(
        sent_txs,
        Duration::from_secs(test_spec.verify_timeout_sec),
    ));
    info!("loadtest completed.");
}

// reads accounts from a file.
fn read_test_spec(filepath: String) -> TestSpec {
    let mut f = File::open(filepath).expect("no input file");
    let mut buffer = String::new();
    f.read_to_string(&mut buffer)
        .expect("failed to read accounts");
    serde_json::from_str(&buffer).expect("failed to parse accounts")
}

// parses and builds new accounts.
fn construct_test_accounts(
    input_accs: &[AccountInfo],
    transport: Http,
    config: &ConfigurationOptions,
) -> Vec<TestAccount> {
    input_accs
        .iter()
        .map(|acc_info| {
            let addr: H160 = acc_info.address.parse().expect("failed to parse address");
            let pk: H256 = acc_info
                .private_key
                .parse()
                .expect("failed to parse private key");
            let eth_acc = EthereumAccount::new(
                pk,
                addr,
                transport.clone(),
                config.contract_eth_addr,
                &config,
            );
            TestAccount {
                zk_acc: ZksyncAccount::new(
                    ZksyncAccount::rand().private_key,
                    0,
                    eth_acc.address,
                    eth_acc.private_key,
                ),
                eth_acc,
                eth_nonce: Mutex::new(0),
            }
        })
        .collect()
}

// sends confugured deposits, withdraws and transfers from each account concurrently.
async fn send_transactions(test_accounts: &[TestAccount], ctx: &TestSpec) -> SentTransactions {
    let sent_txs = SentTransactions::new();
    try_join_all(
        test_accounts
            .iter()
            .enumerate()
            .map(|(i, _)| send_transactions_from_acc(i, &test_accounts, &ctx, &sent_txs))
            .collect::<Vec<_>>(),
    )
    .await
    .expect("[send_transactions]");
    sent_txs
}

// sends configured deposits, withdraws and transfer from a single account concurrently.
async fn send_transactions_from_acc(
    index: usize,
    test_accounts: &[TestAccount],
    ctx: &TestSpec,
    sent_txs: &SentTransactions,
) -> Result<(), failure::Error> {
    let test_acc = &test_accounts[index];
    let addr_hex = hex::encode(test_acc.eth_acc.address);
    update_eth_nonce(test_acc).await?;
    let op_id = deposit_single(test_acc, BigDecimal::from(ctx.deposit_initial)).await?;
    info!("account {} made initial deposit", addr_hex);
    sent_txs.add_op_id(op_id);
    change_pubkey(test_acc).await?;
    let futs_deposits = try_join_all((0..ctx.n_deposits).map(|_i| {
        let amount = rand_amount(ctx.deposit_from_amount, ctx.deposit_to_amount);
        deposit_single(test_acc, amount)
    }));
    let futs_withdraws = try_join_all((0..ctx.n_withdraws).map(|_i| {
        let amount = rand_amount(ctx.withdraw_from_amount, ctx.withdraw_to_amount);
        withdraw_single(test_acc, amount)
    }));
    let futs_transfers = try_join_all((0..ctx.n_transfers).map(|_i| {
        let amount = rand_amount(ctx.transfer_from_amount, ctx.transfer_to_amount);
        transfer_single(index, test_accounts, amount)
    }));
    let (deposit_ids, withdraw_hashes, transfer_hashes) =
        try_join!(futs_deposits, futs_withdraws, futs_transfers)?;
    info!(
        "simultaneous deposits, transfers and withdraws sent for account: {}",
        addr_hex
    );
    sent_txs.add_op_ids(deposit_ids);
    sent_txs.add_tx_hashes(withdraw_hashes);
    sent_txs.add_tx_hashes(transfer_hashes);
    Ok(())
}

// generates random amount for transaction within given range [from, to).
fn rand_amount(from: u64, to: u64) -> BigDecimal {
    let smallest_rand_step = 100_000_000;
    let amount = rand::thread_rng().gen_range(from, to);
    BigDecimal::from(amount - amount % smallest_rand_step)
}

// updates current ethereum nonces from eth node.
async fn update_eth_nonce(test_acc: &TestAccount) -> Result<(), failure::Error> {
    let mut nonce = test_acc.eth_nonce.lock().unwrap();
    let v = test_acc
        .eth_acc
        .main_contract_eth_client
        .pending_nonce()
        .await
        .map_err(|e| failure::format_err!("update_eth_nonce: {}", e))?;
    *nonce = v.as_u32();
    Ok(())
}

async fn change_pubkey(ta: &TestAccount) -> Result<TxHash, failure::Error> {
    send_tx(FranklinTx::ChangePubKey(Box::new(
        ta.zk_acc.create_change_pubkey_tx(None, true, false),
    )))
    .await
}

// deposits to contract and waits for node to execute it.
async fn deposit_single(
    test_acc: &TestAccount,
    deposit_amount: BigDecimal,
) -> Result<u32, failure::Error> {
    let nonce = {
        let mut n = test_acc.eth_nonce.lock().unwrap();
        *n += 1;
        Some(U256::from(*n - 1))
    };
    let po = test_acc
        .eth_acc
        .deposit_eth(deposit_amount, &test_acc.zk_acc.address, nonce)
        .await?;
    let mut executed = false;
    // 5 min wait
    let n_checks = 5 * 60;
    let check_period = Duration::from_secs(1);
    for _i in 0..n_checks {
        let (ex, _) = ethop_info(po.serial_id).await?;
        if ex {
            executed = true;
            break;
        }
        thread::sleep(check_period);
    }
    if executed {
        return Ok(po.serial_id as u32);
    }
    failure::bail!("timeout")
}

// sends withdraw.
async fn withdraw_single(
    test_acc: &TestAccount,
    amount: BigDecimal,
) -> Result<TxHash, failure::Error> {
    let tx = FranklinTx::Withdraw(Box::new(test_acc.zk_acc.sign_withdraw(
        0, // ETH
        amount,
        BigDecimal::from(0),
        &test_acc.eth_acc.address,
        None,
        true,
    )));
    send_tx(tx).await
}

// sends transfer tx to a random receiver.
async fn transfer_single(
    index_from: usize,
    test_accounts: &[TestAccount],
    amount: BigDecimal,
) -> Result<TxHash, failure::Error> {
    let from = &test_accounts[index_from];
    let step = rand::thread_rng().gen_range(1, test_accounts.len());
    let to = &test_accounts[(index_from + step) % test_accounts.len()];
    let tx = FranklinTx::Transfer(Box::new(from.zk_acc.sign_transfer(
        0, // ETH
        amount,
        BigDecimal::from(0),
        &to.zk_acc.address,
        None,
        true,
    )));
    send_tx(tx).await
}

#[derive(Serialize)]
struct SubmitTxMsg {
    id: String,
    method: String,
    jsonrpc: String,
    params: Vec<FranklinTx>,
}

impl SubmitTxMsg {
    fn new(tx: FranklinTx) -> Self {
        Self {
            id: "1".to_owned(),
            method: "tx_submit".to_owned(),
            jsonrpc: "2.0".to_owned(),
            params: vec![tx],
        }
    }
}

// sends tx to server json rpc endpoint.
async fn send_tx(tx: FranklinTx) -> Result<TxHash, failure::Error> {
    let tx_hash = tx.hash();
    let msg = SubmitTxMsg::new(tx);

    let client = reqwest::Client::new();
    let mut res = client
        .post("http://localhost:3030")
        .json(&msg)
        .send()
        .expect("failed to submit tx");
    if res.status() != reqwest::StatusCode::OK {
        failure::bail!("non-ok response: {}", res.status());
    }
    trace!("tx: {}", res.text().unwrap());
    Ok(tx_hash)
}

// waits for all priority operations and transactions to become part of some block and get verified.
async fn wait_for_verify(sent_txs: SentTransactions, timeout: Duration) {
    let start = Instant::now();
    let serial_ids = sent_txs.op_serial_ids.lock().unwrap();
    let sleep_period = Duration::from_millis(500);
    for &id in serial_ids.iter() {
        loop {
            let (executed, verified) = ethop_info(id as u64)
                .await
                .expect("[wait_for_verify] call ethop_info");
            if executed && verified {
                debug!("deposit (serial_id={}) is verified", id);
                break;
            }
            if start.elapsed() > timeout {
                panic!("[wait_for_verify] timeout")
            }
            thread::sleep(sleep_period);
        }
    }
    let tx_hashes = sent_txs.tx_hashes.lock().unwrap();
    for hash in tx_hashes.iter() {
        loop {
            let verified = tx_info(hash.clone())
                .await
                .expect("[wait_for_verify] call tx_info");
            if verified {
                debug!("{} is verified", hash.to_string());
                break;
            }
            if start.elapsed() > timeout {
                panic!("[wait_for_verify] timeout")
            }
            thread::sleep(sleep_period);
        }
    }
}

#[derive(Serialize)]
struct EthopInfo {
    id: String,
    method: String,
    jsonrpc: String,
    params: Vec<u64>,
}

impl EthopInfo {
    fn new(serial_id: u64) -> Self {
        Self {
            id: "3".to_owned(),
            method: "ethop_info".to_owned(),
            jsonrpc: "2.0".to_owned(),
            params: vec![serial_id],
        }
    }
}

// requests and returns a tuple (executed, verified) for operation with given serial_id
async fn ethop_info(serial_id: u64) -> Result<(bool, bool), failure::Error> {
    let msg = EthopInfo::new(serial_id);

    let client = reqwest::Client::new();
    let mut res = client
        .post("http://localhost:3030")
        .json(&msg)
        .send()
        .expect("failed to send ethop_info");
    if res.status() != reqwest::StatusCode::OK {
        failure::bail!("non-ok response: {}", res.status());
    }
    let reply: Output = res.json().unwrap();
    let ret = match reply {
        Output::Success(v) => v.result,
        Output::Failure(v) => panic!("{}", v.error),
    };
    let obj = ret.as_object().unwrap();
    let executed = obj["executed"].as_bool().unwrap();
    if !executed {
        return Ok((false, false));
    }
    let block = obj["block"].as_object().unwrap();
    let verified = block["verified"].as_bool().unwrap();
    Ok((executed, verified))
}

#[derive(Serialize)]
struct TxInfo {
    id: String,
    method: String,
    jsonrpc: String,
    params: Vec<TxHash>,
}

impl TxInfo {
    fn new(h: TxHash) -> Self {
        Self {
            id: "4".to_owned(),
            method: "tx_info".to_owned(),
            jsonrpc: "2.0".to_owned(),
            params: vec![h],
        }
    }
}

// requests and returns whether transaction is verified or not.
async fn tx_info(tx_hash: TxHash) -> Result<bool, failure::Error> {
    let msg = TxInfo::new(tx_hash);

    let client = reqwest::Client::new();
    let mut res = client
        .post("http://localhost:3030")
        .json(&msg)
        .send()
        .expect("failed to send tx_info");
    if res.status() != reqwest::StatusCode::OK {
        failure::bail!("non-ok response: {}", res.status());
    }
    let reply: Output = res.json().unwrap();
    let ret = match reply {
        Output::Success(v) => v.result,
        Output::Failure(v) => panic!("{}", v.error),
    };
    let obj = ret.as_object().unwrap();
    let executed = obj["executed"].as_bool().unwrap();
    if !executed {
        return Ok(false);
    }
    let block = obj["block"].as_object().unwrap();
    let verified = block["verified"].as_bool().unwrap();
    Ok(verified)
}
