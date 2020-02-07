use bigdecimal::BigDecimal;
use futures::executor::block_on;
use futures::future::try_join_all;
use models::config_options::ConfigurationOptions;
use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use testkit::eth_account::{parse_ether, EthereumAccount};
use testkit::zksync_account::ZksyncAccount;
use web3::transports::Http;
use web3::types::{H160, H256};

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
    let config = ConfigurationOptions::from_env();
    let filepath = env::args().nth(1).expect("account.json path not given");
    let input_accs = read_accounts(filepath);
    let (_el, transport) = Http::new(&config.web3_url).expect("http transport start");
    let test_accounts = construct_test_accounts(input_accs, transport, &config);
    let deposit_amount = parse_ether("0.00001").expect("failed to parse ETH");
    block_on(do_deposits(&test_accounts[..], deposit_amount));
    println!("End");
}

async fn do_deposits(test_accounts: &[TestAccount], deposit_amount: BigDecimal) {
    try_join_all(
        test_accounts
            .iter()
            .map(|test_acc| deposit_single(&test_acc, deposit_amount.clone()))
            .collect::<Vec<_>>(),
    )
    .await
    .expect("failed to deposit");
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
