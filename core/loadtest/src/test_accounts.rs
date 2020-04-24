// Built-in import
// External uses
use num::BigUint;
use rand::Rng;
use tokio::sync::Mutex;
use web3::transports::Http;
// Workspace uses
use models::{
    config_options::ConfigurationOptions,
    node::{tx::PackedEthSignature, FranklinTx},
};
use testkit::{eth_account::EthereumAccount, zksync_account::ZksyncAccount};
// Local uses
use crate::{rpc_client::RpcClient, test_spec::AccountInfo};

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

    // Updates the current Ethereum and ZKSync account nonce values.
    pub async fn update_nonce_values(&self, rpc_client: &RpcClient) -> Result<(), failure::Error> {
        // Update ETH nonce.
        let mut nonce = self.eth_nonce.lock().await;
        let v = self
            .eth_acc
            .main_contract_eth_client
            .pending_nonce()
            .await
            .map_err(|e| failure::format_err!("update_eth_nonce: {}", e))?;
        *nonce = v.as_u32();

        // Update ZKSync nonce.
        let zknonce = rpc_client
            .account_state_info(self.zk_acc.address)
            .await
            .expect("rpc error")
            .committed
            .nonce;
        self.zk_acc.set_nonce(zknonce);

        Ok(())
    }

    pub fn sign_change_pubkey(&self) -> FranklinTx {
        FranklinTx::ChangePubKey(Box::new(
            self.zk_acc.create_change_pubkey_tx(None, true, false),
        ))
    }

    // Creates a signed withdraw transaction.
    pub fn sign_withdraw_single(
        &self,
        amount: BigUint,
    ) -> (FranklinTx, Option<PackedEthSignature>) {
        let (tx, eth_signature) = self.zk_acc.sign_withdraw(
            0, // ETH
            "ETH",
            amount,
            BigUint::from(0u32),
            &self.eth_acc.address,
            None,
            true,
        );
        (FranklinTx::Withdraw(Box::new(tx)), Some(eth_signature))
    }

    // Creates a signed transfer tx to a random receiver.
    pub fn sign_transfer_to_random(
        &self,
        test_accounts: &[AccountInfo],
        amount: BigUint,
    ) -> (FranklinTx, Option<PackedEthSignature>) {
        let to = {
            let mut to_idx = rand::thread_rng().gen_range(0, test_accounts.len() - 1);
            while test_accounts[to_idx].address == self.zk_acc.address {
                to_idx = rand::thread_rng().gen_range(0, test_accounts.len() - 1);
            }
            test_accounts[to_idx].address
        };
        let (tx, eth_signature) = self.zk_acc.sign_transfer(
            0, // ETH
            "ETH",
            amount,
            BigUint::from(0u32),
            &to,
            None,
            true,
        );
        (FranklinTx::Transfer(Box::new(tx)), Some(eth_signature))
    }
}
