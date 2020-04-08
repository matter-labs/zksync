// Built-in import
// External uses
use tokio::sync::Mutex;
use web3::transports::Http;
// Workspace uses
use models::config_options::ConfigurationOptions;
use testkit::{eth_account::EthereumAccount, zksync_account::ZksyncAccount};
// Local uses
use super::test_spec::AccountInfo;

#[derive(Debug)]
pub struct TestAccount {
    pub zk_acc: ZksyncAccount,
    pub eth_acc: EthereumAccount<Http>,
    pub eth_nonce: Mutex<u32>,
}

impl TestAccount {
    // Parses and builds a new accounts list.
    pub fn construct_test_accounts(
        input_accs: &[AccountInfo],
        transport: Http,
        config: &ConfigurationOptions,
    ) -> Vec<Self> {
        input_accs
            .iter()
            .map(|acc_info| {
                let addr = acc_info.address;
                let pk = acc_info.private_key;
                let eth_acc = EthereumAccount::new(
                    pk,
                    addr,
                    transport.clone(),
                    config.contract_eth_addr,
                    &config,
                );
                Self {
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
}
