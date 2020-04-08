//! Load test aims the following purposes:
//! - spamming the node with a big amount of transactions to simulate the big pressure;
//! - measuring the metrics of the node txs processing progress;
//! - quick filling the node's database with a lot of the real-world data.
//!
//! The behavior of the loadtest is flexible and determined by different "scenarios":
//! every scenario is basically a function which interacts with a node according to some rules.
//! Currently the main scenario is a "basic" scenario, which creates transactions according to the
//! test spec parameters parsed from a `json` file.

// Built-in import
use std::env;
// External uses
use tokio::runtime::Builder;
// Workspace uses
use models::config_options::ConfigurationOptions;
// Local uses
use self::{scenarios::basic_scenario::basic_scenario, scenarios::ScenarioContext};

mod rpc_client;
mod scenarios;
mod sent_transactions;
mod test_accounts;
mod test_spec;
mod tps_counter;

fn main() {
    env_logger::init();
    let tokio_runtime = Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .expect("failed to construct tokio runtime");

    let config = ConfigurationOptions::from_env();
    let test_spec_path = env::args().nth(1).expect("test spec file not given");
    let rpc_addr = env::var("HTTP_RPC_API_ADDR").expect("HTTP_RPC_API_ADDR is missing");

    let context = ScenarioContext::new(config, test_spec_path, rpc_addr.clone(), tokio_runtime);

    basic_scenario(context);
}
