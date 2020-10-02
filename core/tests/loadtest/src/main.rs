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
use structopt::StructOpt;
// External uses
use tokio::runtime::Builder;
// Workspace uses
use zksync::{Network, Provider};
use zksync_config::ConfigurationOptions;
// Local uses
use self::{
    cli::CliOptions, monitor::Monitor, scenarios::configs::AccountInfo, scenarios::ScenarioContext,
};

mod cli;
mod monitor;
mod ng;
mod scenarios;
mod sent_transactions;
mod test_accounts;
mod tps_counter;

fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    let mut tokio_runtime = Builder::new().threaded_scheduler().enable_all().build()?;

    let env_config = ConfigurationOptions::from_env();
    let monitor = Monitor::new(Provider::new(Network::Localhost));
    let main_account = AccountInfo {
        address: "36615Cf349d7F6344891B1e7CA7C72883F5dc049".parse()?,
        private_key: "7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110".parse()?,
    };
    let scenario = ng::scenarios::SimpleScenario {
        transfer_size: 100_u64.into(),
        wallets: 100,
    };
    tokio_runtime.block_on(scenario.run(monitor, main_account, env_config))?;

    // .run(monitor, main_account, env_config)

    // let CliOptions {
    //     test_spec_path,
    //     scenario_type,
    // } = CliOptions::from_args();

    // let provider = Provider::new(Network::Localhost);
    // let context = ScenarioContext::new(provider, env_config, test_spec_path, tokio_runtime);

    // let scenario = scenario_type.into_scenario();

    // scenario(context);

    Ok(())
}
