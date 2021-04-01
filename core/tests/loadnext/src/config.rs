#[derive(Debug, Clone)]
pub struct LoadtestConfig {
    pub zksync_rpc_addr: String,
    pub web3_url: String,

    pub master_wallet_pk: String,

    pub accounts_amount: usize,
    pub operations_per_account: usize,

    pub main_token: String,
}
