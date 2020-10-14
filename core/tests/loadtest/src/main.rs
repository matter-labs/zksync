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
use std::path::PathBuf;
// External uses
use structopt::StructOpt;
// Workspace uses
use zksync_config::ConfigurationOptions;
// Local uses
use loadtest::{Config, ScenarioExecutor};

/// An utility for simulating a load similar to a real one.
#[derive(Debug, StructOpt)]
#[structopt(rename_all = "kebab-case")]
struct LoadtestOpts {
    /// Path to a load test configuration file.
    #[structopt(short = "p", long)]
    config_path: Option<PathBuf>,
    /// Print the results as json file.
    #[structopt(long)]
    json_output: bool,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init();
    let env_config = ConfigurationOptions::from_env();

    let opts = LoadtestOpts::from_args();

    let config = opts
        .config_path
        .map(Config::from_toml)
        .transpose()?
        .unwrap_or_default();

    let executor = ScenarioExecutor::new(config, env_config).await?;
    let journal = executor.run().await?;

    let summary = journal.five_stats_summary()?;
    if opts.json_output {
        println!("{}", serde_json::to_string_pretty(&summary)?);
    } else {
        todo!()
    }

    Ok(())
}
