//! Load test aims the following purposes:
//! - spamming the node with a big amount of transactions to simulate the big pressure;
//! - measuring the metrics of the node txs processing progress;
//! - quick filling the node's database with a lot of the real-world data.
//!
//! The behavior of the loadtest is flexible and determined by different "scenarios":
//! every scenario is basically a function which interacts with a node according to some rules.
//!
//! Currently supported scenarios:
//!
//! - Outgoing TPS. Measures the throughput of the ZKSync node's mempool (time of the tx acceptance).
//!   To run this scenario, use the following command:
//!   
//!   ```sh
//!   f cargo run --release --bin loadtest -- --scenario outgoing core/loadtest/src/loadtest.json
//!   ```
//!   
//! - Execution TPS. Measures the throughput of the ZKSync block executor (amount of txs executed per second)
//!   To run this scenario, use the following command:
//!   
//!   ```sh
//!   f cargo run --release --bin loadtest -- --scenario execution core/loadtest/src/loadtest.json
//!   ```

// Built-in import
use std::env;
use structopt::StructOpt;
// External uses
use tokio::runtime::Builder;
// Workspace uses
use models::config_options::ConfigurationOptions;
// Local uses
use self::{cli::CliOptions, scenarios::ScenarioContext};

mod cli;
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

    let rpc_addr = env::var("HTTP_RPC_API_ADDR").expect("HTTP_RPC_API_ADDR is missing");
    let env_config = ConfigurationOptions::from_env();
    let CliOptions {
        test_spec_path,
        scenario_type,
    } = CliOptions::from_args();

    let context = ScenarioContext::new(env_config, test_spec_path, rpc_addr, tokio_runtime);

    let scenario = scenario_type.into_scenario();

    scenario(context);
}
