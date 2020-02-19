use bigdecimal::BigDecimal;
use futures::executor::block_on;
use futures::future::{join_all, try_join_all};
use futures::join;
use jsonrpc_core::types::response::Success;
use jsonrpc_core::types::Value;
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
use testkit::eth_account::{parse_ether, EthereumAccount};
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
    eth_nonce: Mutex<U256>,
}

fn main() {
    env_logger::init();

    let deposit_initial = parse_ether("1.0").expect("failed to parse");
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
    let input_accs2 = read_accounts(filepath);
    let (_el, transport) = Http::new(&config.web3_url).expect("http transport start");
    let test_accounts = Arc::new(construct_test_accounts(
        input_accs,
        transport.clone(),
        &config,
    ));
    let test_accounts2 = Arc::clone(&test_accounts);
    let deposit_amount = parse_ether("1.0").expect("failed to parse");
    let transfer_amount = parse_ether("0.1").expect("failed to parse");
    let withdraw_amount = parse_ether("0.2").expect("failed to parse");
    info!("Inital depsoits");
    block_on(do_deposits(&test_accounts[..], deposit_amount.clone()));
    info!("done [Inital depsoits].");
    let h = thread::spawn(move || {
        info!("Simultaneous transfers and withdraws");
        block_on(async {
            join!(
                join_all((0..n_transfers).map(|_i| {
                    let v = rand::thread_rng().gen_range(transfer_from_amount, transfer_to_amount);
                    let v = parse_ether(&v.to_string()).expect("parse error");
                    do_transfers(&test_accounts2[..], v)
                })),
                join_all((0..n_withdraws).map(|_i| {
                    let v = rand::thread_rng().gen_range(withdraw_from_amount, withdraw_to_amount);
                    let v = parse_ether(&v.to_string()).expect("parse error");
                    do_withdraws(&test_accounts2[..], v)
                }))
            )
        });
        info!("done [Simultaneous transfers and withdraws].")
    });
    info!("deposits");
    block_on(async {
        join_all((0..n_deposits).map(|_i| {
            let v = rand::thread_rng().gen_range(deposit_from_amount, deposit_to_amount);
            let v = parse_ether(&v.to_string()).expect("parse error");
            do_deposits(&test_accounts[..], v)
        }))
    });
    info!("done [deposits].");
    h.join().unwrap();
    // TODO: final checks
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
            let mut ta = TestAccount {
                zk_acc: ZksyncAccount::new(
                    ZksyncAccount::rand().private_key,
                    0,
                    eth_acc.address,
                    eth_acc.private_key,
                ),
                eth_acc,
                eth_nonce: Mutex::new(U256::from(0)),
            };
            update_eth_nonce(&mut ta);
            ta
        })
        .collect()
}

fn update_eth_nonce(ta: &mut TestAccount) {
    let mut nonce = ta.eth_nonce.lock().unwrap();
    *nonce = block_on(ta.eth_acc.main_contract_eth_client.pending_nonce()).unwrap()
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
    let nonce = {
        let mut nonce = test_acc.eth_nonce.lock().unwrap();
        let v = *nonce;
        *nonce = U256::from(v.as_u32() + 1);
        U256::from(v.as_u32())
    };
    let po = test_acc
        .eth_acc
        .deposit_eth(deposit_amount, &test_acc.zk_acc.address, Some(nonce))
        .await?;
    let mut verified = false;
    // 5 min wait
    let n_checks = 5 * 60;
    let check_period = std::time::Duration::from_secs(1);
    for _i in 0..n_checks {
        thread::sleep(check_period);
        let ret = ethop_info(po.serial_id).await?;
        println!("{:?}", ret);
        let obj = ret.result.as_object().unwrap();
        if !obj["block"].is_object() {
            continue;
        }
        let block = obj["block"].as_object().unwrap();
        if block["verified"].is_boolean() && block["verified"].as_bool().unwrap() {
            verified = true;
            break;
        }
    }
    if verified {
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

async fn do_transfers(test_accounts: &[TestAccount], deposit_amount: BigDecimal) {
    try_join_all(
        test_accounts
            .iter()
            .enumerate()
            .map(|(i, _)| {
                let from = &test_accounts[i];
                let to = &test_accounts[(i + 1) % test_accounts.len()];
                let tx = FranklinTx::Transfer(Box::new(from.zk_acc.sign_transfer(
                    0, // ETH
                    deposit_amount.clone(),
                    BigDecimal::from(0),
                    &to.zk_acc.address,
                    None,
                    true,
                )));
                send_tx(tx)
            })
            .collect::<Vec<_>>(),
    )
    .await
    .expect("failed to do transfers");
}

async fn do_withdraws(test_accounts: &[TestAccount], deposit_amount: BigDecimal) {
    trace!("start do_withdraws");
    try_join_all(
        test_accounts
            .iter()
            .map(|test_acc| {
                let tx = FranklinTx::Withdraw(Box::new(test_acc.zk_acc.sign_withdraw(
                    0, // ETH
                    deposit_amount.clone(),
                    BigDecimal::from(0),
                    &test_acc.eth_acc.address,
                    None,
                    true,
                )));
                send_tx(tx)
            })
            .collect::<Vec<_>>(),
    )
    .await
    .expect("failed to do withdraws");
    trace!("end do_withdraws");
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
