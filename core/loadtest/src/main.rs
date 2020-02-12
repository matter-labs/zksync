use bigdecimal::BigDecimal;
use futures::executor::block_on;
use futures::future::try_join_all;
use futures::join;
use log::{info, trace};
use models::config_options::ConfigurationOptions;
use models::node::tx::FranklinTx;
use serde::Deserialize;
use serde::Serialize;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use testkit::eth_account::{parse_ether, EthereumAccount};
use testkit::zksync_account::ZksyncAccount;
use tokio::runtime::Runtime;
use web3::transports::Http;
use web3::types::{H160, H256};

// TODO: use dynamic size.
const N_ACC: usize = 10;

#[derive(Deserialize, Debug)]
struct AccountInfo {
    pub address: String,
    pub private_key: String,
}

struct TestAccount {
    zk_acc: ZksyncAccount,
    eth_acc: EthereumAccount<Http>,
}

fn main() {
    env_logger::init();
    let _runtime = Runtime::new().unwrap();
    let config = ConfigurationOptions::from_env();
    let filepath = env::args().nth(1).expect("account.json path not given");
    let input_accs = read_accounts(filepath);
    let (_el, transport) = Http::new(&config.web3_url).expect("http transport start");
    let test_accounts = construct_test_accounts(input_accs, transport, &config);
    let deposit_amount = parse_ether("1.0").expect("failed to parse");
    let transfer_amount = parse_ether("0.1").expect("failed to parse");
    let withdraw_amount = parse_ether("0.2").expect("failed to parse");
    info!("Inital depsoits");
    block_on(do_deposits(&test_accounts[..], deposit_amount.clone()));
    info!("done.");
    info!("Simultaneous tranfers and withdraws");
    block_on(async {
        join!(
            // 5 tranfers for each account
            do_transfers(&test_accounts[..], transfer_amount.clone()),
            do_transfers(&test_accounts[..], transfer_amount.clone()),
            do_transfers(&test_accounts[..], transfer_amount.clone()),
            do_transfers(&test_accounts[..], transfer_amount.clone()),
            do_transfers(&test_accounts[..], transfer_amount.clone()),
            // 2 withdraws for each account
            do_withdraws(&test_accounts[..], withdraw_amount.clone()),
            do_withdraws(&test_accounts[..], withdraw_amount.clone()),
        )
    });
    info!("done.");
    info!("Final withdraws");
    let left_balance = parse_ether("0.6").expect("failed to parse");
    block_on(do_withdraws(&test_accounts[..], left_balance));
    info!("done.");
    info!("End");
}

fn read_accounts(filepath: String) -> [AccountInfo; N_ACC] {
    let mut f = File::open(filepath).expect("no input file");
    let mut buffer = String::new();
    f.read_to_string(&mut buffer)
        .expect("failed to read accounts");
    serde_json::from_str(&buffer).expect("failed to parse accounts")
}

fn construct_test_accounts(
    input_accs: [AccountInfo; N_ACC],
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
                zk_acc: ZksyncAccount::rand(),
                eth_acc,
            }
        })
        .collect()
}

async fn do_deposits(test_accounts: &[TestAccount], deposit_amount: BigDecimal) {
    trace!("start do_deposits");
    try_join_all(
        test_accounts
            .iter()
            .map(|test_acc| deposit_single(&test_acc, deposit_amount.clone()))
            .collect::<Vec<_>>(),
    )
    .await
    .expect("failed to deposit");
    trace!("end do_deposits");
}

async fn deposit_single(
    test_acc: &TestAccount,
    deposit_amount: BigDecimal,
) -> Result<(), failure::Error> {
    test_acc
        .eth_acc
        .deposit_eth(deposit_amount, &test_acc.zk_acc.address)
        .await?;
    Ok(())
}

async fn do_transfers(test_accounts: &[TestAccount], deposit_amount: BigDecimal) {
    try_join_all(
        test_accounts
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let from = &test_accounts[i];
                let to = &test_accounts[(i + 1) % test_accounts.len()];
                let tx = FranklinTx::Transfer(from.zk_acc.sign_transfer(
                    0, // ETH
                    deposit_amount.clone(),
                    BigDecimal::from(0),
                    &to.zk_acc.address,
                    None,
                    true,
                ));
                send_tx(tx)
            })
            .collect::<Vec<_>>(),
    )
    .await
    .expect("failed to do transfers");
}

#[derive(Serialize)]
struct SubmitTxMsg {
    id: String,
    method: String,
    jsonrpc: String,
    params: Vec<FranklinTx>,
}

async fn do_withdraws(test_accounts: &[TestAccount], deposit_amount: BigDecimal) {
    trace!("start do_withdraws");
    try_join_all(
        test_accounts
            .iter()
            .map(|test_acc| {
                let tx = FranklinTx::Withdraw(test_acc.zk_acc.sign_withdraw(
                    0, // ETH
                    deposit_amount.clone(),
                    BigDecimal::from(0),
                    &test_acc.eth_acc.address,
                    None,
                    true,
                ));
                send_tx(tx)
            })
            .collect::<Vec<_>>(),
    )
    .await
    .expect("failed to do withdraws");
    trace!("end do_withdraws");
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
