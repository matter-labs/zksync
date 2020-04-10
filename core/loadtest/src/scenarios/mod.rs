//! Module with different scenarios for a `loadtest`.
//! A scenario is basically is a behavior policy for sending the transactions.
//! A simplest scenario will be: "get a bunch of accounts and just spawn a lot of transfer
//! operations between them".

// Built-in import
use std::{path::PathBuf, str::FromStr, sync::Arc};
// External uses
use tokio::runtime::Runtime;
use web3::transports::{EventLoopHandle, Http};
// Workspace uses
use models::config_options::ConfigurationOptions;
// Local uses
use super::{test_accounts::TestAccount, test_spec::TestSpec, tps_counter::TPSCounter};

mod execution_tps;
mod outgoing_tps;

pub type Scenario = Box<dyn Fn(ScenarioContext)>;

/// Supported scenario types.
#[derive(Debug, Clone, Copy)]
pub enum ScenarioType {
    /// Measure the outgoing TPS (ZKSync node mempool acceptance throughput).
    OutgoingTps,
    /// Measure the TPS for transactions execution (not including verifying).
    ExecutionTps,
}

impl ScenarioType {
    /// Returns the scenario function given its type.
    pub fn into_scenario(self) -> Scenario {
        match self {
            Self::OutgoingTps => Box::new(outgoing_tps::run_scenario),
            Self::ExecutionTps => Box::new(execution_tps::run_scenario),
        }
    }
}

impl FromStr for ScenarioType {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let scenario = match s {
            "outgoing" | "outgoing_tps" => Self::OutgoingTps,
            "execution" | "execution_tps" => Self::ExecutionTps,
            other => {
                failure::bail!(
                    "Unknown scenario type '{}'. \
                     Available options are: \
                     'outgoing_tps', 'execution_tps'",
                    other
                );
            }
        };

        Ok(scenario)
    }
}

#[derive(Debug)]
pub struct ScenarioContext {
    // Handle for the `web3` transport, which must not be `drop`ped for transport to work.
    _event_loop_handle: EventLoopHandle,
    pub test_accounts: Vec<TestAccount>,
    pub ctx: TestSpec,
    pub rpc_addr: String,
    pub tps_counter: Arc<TPSCounter>,
    pub rt: Runtime,
}

impl ScenarioContext {
    pub fn new(
        config: ConfigurationOptions,
        test_spec_path: PathBuf,
        rpc_addr: String,
        rt: Runtime,
    ) -> Self {
        // Load the test spec.
        let ctx = TestSpec::load(test_spec_path);

        // Create test accounts.
        let (_event_loop_handle, transport) =
            Http::new(&config.web3_url).expect("http transport start");
        let test_accounts =
            TestAccount::construct_test_accounts(&ctx.input_accounts, transport, &config);

        let tps_counter = Arc::new(TPSCounter::default());

        Self {
            _event_loop_handle,
            test_accounts,
            ctx,
            rpc_addr,
            tps_counter,
            rt,
        }
    }
}
