use ff::{PrimeField, PrimeFieldRepr};
use franklin_crypto::alt_babyjubjub::fs::FsRepr;
use franklin_crypto::eddsa::PrivateKey;
use models::config_options::ConfigurationOptions;
use models::node::{Engine, Fs};
use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use testkit::eth_account::EthereumAccount;
use testkit::zksync_account::ZksyncAccount;
use testkit::{AccountSet, ZKSyncAccountId};
use web3::transports::Http;
use web3::types::{H160, H256};

const N_ACC: usize = 10;

#[derive(Deserialize, Debug)]
struct AccountInfo {
    pub address: String,
    pub private_key: String,
}

fn main() {
    let config = ConfigurationOptions::from_env();
    let filepath = env::args().nth(1).expect("account.json path not given");
    let input_accs = read_accounts(filepath);
    let account_set = create_accountset(input_accs, &config);
}

fn read_accounts(filepath: String) -> [AccountInfo; N_ACC] {
    let mut f = File::open(filepath).expect("no input file");
    let mut buffer = String::new();
    f.read_to_string(&mut buffer)
        .expect("failed to read accounts");
    serde_json::from_str(&buffer).expect("failed to parse accounts")
}

fn create_accountset(
    input_accs: [AccountInfo; N_ACC],
    config: &ConfigurationOptions,
) -> AccountSet<Http> {
    let (_el, transport) = Http::new(&config.web3_url).expect("http transport start");
    let eth_accounts = input_accs
        .into_iter()
        .map(|acc_info| {
            let addr: H160 = acc_info.address.parse().expect("failed to parse address");
            let pk: H256 = acc_info
                .private_key
                .parse()
                .expect("failed to parse private key");
            EthereumAccount::new(
                pk,
                addr,
                transport.clone(),
                config.contract_eth_addr,
                &config,
            )
        })
        .collect::<Vec<_>>();
    let fee_account = ZksyncAccount::rand();
    let zksync_accounts = {
        let mut zksync_accounts = Vec::new();
        zksync_accounts.push(fee_account);
        zksync_accounts.extend(eth_accounts.iter().map(|_eth_account| {
            let rng_zksync_key = ZksyncAccount::rand().private_key;
            ZksyncAccount::new(rng_zksync_key, 0)
        }));
        zksync_accounts
    };
    AccountSet {
        eth_accounts,
        zksync_accounts,
        fee_account_id: ZKSyncAccountId(0),
    }
}

fn hex_to_private_key(s: &str) -> PrivateKey<Engine> {
    let data = hex::decode(s).unwrap();
    let mut fs_repr = FsRepr::default();
    fs_repr.read_be(&data[..]).unwrap();
    PrivateKey::<Engine>(Fs::from_repr(fs_repr).unwrap())
}
