//! Module with different scenarios for a `loadtest`.
//! A scenario is basically is a behavior policy for sending the transactions.
//! A simplest scenario will be: "get a bunch of accounts and just spawn a lot of transfer
//! operations between them".

// Built-in import
use std::sync::Arc;
// External uses
use tokio::runtime::Runtime;
use web3::transports::Http;
// Workspace uses
use models::config_options::ConfigurationOptions;
// Local uses
use super::{test_accounts::TestAccount, test_spec::TestSpec, tps_counter::TPSCounter};

pub mod basic_scenario;

#[derive(Debug)]
pub struct ScenarioContext {
    pub test_accounts: Vec<TestAccount>,
    pub ctx: TestSpec,
    pub rpc_addr: String,
    pub tps_counter: Arc<TPSCounter>,
    pub rt: Runtime,
}

impl ScenarioContext {
    pub fn new(
        config: ConfigurationOptions,
        test_spec_path: String,
        rpc_addr: String,
        rt: Runtime,
    ) -> Self {
        // Load the test spec.
        let ctx = TestSpec::load(test_spec_path);

        // Create test accounts.
        let (_el, transport) = Http::new(&config.web3_url).expect("http transport start");
        let test_accounts =
            TestAccount::construct_test_accounts(&ctx.input_accounts, transport, &config);

        let tps_counter = Arc::new(TPSCounter::default());

        Self {
            test_accounts,
            ctx,
            rpc_addr,
            tps_counter,
            rt,
        }
    }
}
