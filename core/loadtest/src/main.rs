use bigdecimal::BigDecimal;
use futures::executor::block_on;
use futures::future::try_join_all;
use futures::try_join;
use jsonrpc_core::types::response::Success;
use log::{info, trace};
use models::config_options::ConfigurationOptions;
use models::node::tx::FranklinTx;
use rand::Rng;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::sync::Arc;
use std::sync::Mutex;
use std::thread;
use testkit::eth_account::EthereumAccount;
use testkit::zksync_account::ZksyncAccount;
use web3::transports::Http;
use web3::types::U256;
use web3::types::{H160, H256};

#[derive(Deserialize, Debug)]
struct AccountInfo {
    pub address: String,
    pub private_key: String,
}

struct TestAccount {
    zk_acc: ZksyncAccount,
    eth_acc: EthereumAccount<Http>,
    eth_nonce: Mutex<u32>,
}

struct TestContext {
    deposit_initial: f64,
    n_deposits: i32,
    deposit_from_amount: f64,
    deposit_to_amount: f64,
    n_transfers: i32,
    transfer_from_amount: f64,
    transfer_to_amount: f64,
    n_withdraws: i32,
    withdraw_from_amount: f64,
    withdraw_to_amount: f64,
}

fn main() {
    env_logger::init();

    let deposit_initial = 1.0;
    let n_transfers = 5;
    let n_withdraws = 2;
    let n_deposits = 2;
    let deposit_from_amount = 0.1;
    let deposit_to_amount = 1.0;
    let transfer_from_amount = 0.01;
    let transfer_to_amount = 0.1;
    let withdraw_from_amount = 0.01;
    let withdraw_to_amount = 0.2;
    let config = ConfigurationOptions::from_env();
    let filepath = env::args().nth(1).expect("account.json path not given");
    let input_accs = read_accounts(filepath.clone());
    let (_el, transport) = Http::new(&config.web3_url).expect("http transport start");
    let test_accounts = Arc::new(construct_test_accounts(
        input_accs,
        transport.clone(),
        &config,
    ));
    block_on(send_transactions(
        &test_accounts,
        TestContext {
            deposit_initial,
            n_deposits,
            deposit_from_amount,
            deposit_to_amount,
            n_transfers,
            transfer_from_amount,
            transfer_to_amount,
            n_withdraws,
            withdraw_from_amount,
            withdraw_to_amount,
        },
    ));
    info!("loadtest completed.");
}

fn read_accounts(filepath: String) -> Vec<AccountInfo> {
    let mut f = File::open(filepath).expect("no input file");
    let mut buffer = String::new();
    f.read_to_string(&mut buffer)
        .expect("failed to read accounts");
    serde_json::from_str(&buffer).expect("failed to parse accounts")
}

fn construct_test_accounts(
    input_accs: Vec<AccountInfo>,
    transport: Http,
    config: &ConfigurationOptions,
) -> Vec<TestAccount> {
    input_accs
        .into_iter()
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

async fn send_transactions(test_accounts: &Vec<TestAccount>, ctx: TestContext) {
    try_join_all(
        test_accounts
            .iter()
            .enumerate()
            .map(|(i, _)| send_transactions_from_acc(i, &test_accounts, &ctx))
            .collect::<Vec<_>>(),
    )
    .await
    .expect("[send_transactions]");
}

async fn send_transactions_from_acc(
    index: usize,
    test_accounts: &Vec<TestAccount>,
    ctx: &TestContext,
) -> Result<(), failure::Error> {
    let test_acc = &test_accounts[index];
    deposit_single(test_acc, BigDecimal::from(ctx.deposit_initial)).await?;
    change_pubkey(test_acc).await?;
    update_eth_nonce(test_acc).await?;
    let futs_deposits = try_join_all((0..ctx.n_deposits).map(|_i| {
        let amount = rand::thread_rng().gen_range(ctx.deposit_from_amount, ctx.deposit_to_amount);
        deposit_single(test_acc, BigDecimal::from(amount))
    }));
    let futs_withdraws = try_join_all((0..ctx.n_withdraws).map(|_i| {
        let amount = rand::thread_rng().gen_range(ctx.withdraw_from_amount, ctx.withdraw_to_amount);
        withdraw_single(test_acc, BigDecimal::from(amount))
    }));
    let futs_transfers = try_join_all((0..ctx.n_transfers).map(|_i| {
        let amount = rand::thread_rng().gen_range(ctx.transfer_from_amount, ctx.transfer_to_amount);
        transfer_single(index, test_accounts, BigDecimal::from(amount))
    }));
    try_join!(futs_deposits, futs_withdraws, futs_transfers)?;
    Ok(())
}

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

async fn change_pubkey(ta: &TestAccount) -> Result<(), failure::Error> {
    send_tx(FranklinTx::ChangePubKey(Box::new(
        ta.zk_acc.create_change_pubkey_tx(None, true, false),
    )))
    .await
}

async fn deposit_single(
    test_acc: &TestAccount,
    deposit_amount: BigDecimal,
) -> Result<(), failure::Error> {
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
    let check_period = std::time::Duration::from_secs(1);
    for _i in 0..n_checks {
        let ret = ethop_info(po.serial_id).await?;
        let obj = ret.result.as_object().unwrap();
        executed = obj["executed"].as_bool().unwrap();
        if executed {
            break;
        }
        thread::sleep(check_period);
    }
    if executed {
        return Ok(());
    }
    failure::bail!("timeout")
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

async fn ethop_info(serial_id: u64) -> Result<Success, failure::Error> {
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
    Ok(res.json().unwrap())
}

async fn withdraw_single(test_acc: &TestAccount, amount: BigDecimal) -> Result<(), failure::Error> {
    let tx = FranklinTx::Withdraw(Box::new(test_acc.zk_acc.sign_withdraw(
        0, // ETH
        BigDecimal::from(amount),
        BigDecimal::from(0),
        &test_acc.eth_acc.address,
        None,
        true,
    )));
    send_tx(tx).await
}

async fn transfer_single(
    index_from: usize,
    test_accounts: &Vec<TestAccount>,
    amount: BigDecimal,
) -> Result<(), failure::Error> {
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

async fn send_tx(tx: FranklinTx) -> Result<(), failure::Error> {
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
    Ok(())
}

// TODO: Use below code for final assertions.

#[derive(Serialize)]
struct GetAccountStateMsg {
    id: String,
    method: String,
    jsonrpc: String,
    params: Vec<String>,
}

impl GetAccountStateMsg {
    fn new(addr: &str) -> Self {
        Self {
            id: "2".to_owned(),
            method: "account_info".to_owned(),
            jsonrpc: "2.0".to_owned(),
            params: vec![addr.to_owned()],
        }
    }
}

fn get_account_state(addr: &str) -> server::api_server::rpc_server::AccountInfoResp {
    let msg = GetAccountStateMsg::new(addr);

    let client = reqwest::Client::new();
    let mut resp = client
        .post("http://localhost:3030")
        .json(&msg)
        .send()
        .expect("failed to send request");
    if resp.status() != reqwest::StatusCode::OK {
        panic!("non-ok response: {}", resp.status());
    }
    resp.json().unwrap()
}
