use serde::Deserialize;

/// Configuration for the loadtest.
///
/// This structure is meant to provide the least possible amount of parameters:
/// By the ideology of the test, it is OK for it to be opinionated. Thus we don't provide
/// kinds of operations we want to perform, do not configure fail or pass criteria.
///
/// It is expected that the user will provide the basic settings, and the loadtest will
/// take care of everything else.
#[derive(Debug, Clone, Deserialize)]
pub struct LoadtestConfig {
    /// Address of the zkSync node.
    pub zksync_rpc_addr: String,
    /// Address of the Ethereum web3 API.
    pub web3_url: String,
    /// Used Ethereum network (e.g. `rinkeby` or `localhost`).
    pub eth_network: String,

    /// Ethereum private key of the wallet that has funds to perform a test.
    pub master_wallet_pk: String,

    /// Amount of accounts to be used in test.
    /// This option configures the "width" of the test:
    /// how many concurrent operation flows will be executed.
    pub accounts_amount: usize,
    /// Amount of operations per account.
    /// This option configures the "length" of the test:
    /// how many individual operations each account of the test will execute.
    pub operations_per_account: usize,

    /// Symbolic representation of the ERC-20 token to be used in test.
    ///
    /// Token must satisfy two criteria:
    /// - Be supported by zkSync.
    /// - Have `mint` operation.
    ///
    /// Note that we use ERC-20 token since we can't easily mint a lot of ETH on
    /// Rinkeby or Ropsten without caring about collecting it back.
    pub main_token: String,
}

impl LoadtestConfig {
    pub fn from_env() -> envy::Result<Self> {
        envy::from_env()
    }
}
