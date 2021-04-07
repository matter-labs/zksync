//! Loadtest: an utility to stress-test the zkSync server.
//!
//! In order to launch it, you must provide required environmental variables, for details see `README.md`.
//! Without required variables provided, test is launched in the localhost/development mode with some hard-coded
//! values to check the local zkSync deployment.

use loadnext::{config::LoadtestConfig, executor::Executor, report_collector::LoadtestResult};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    vlog::init();

    let config = LoadtestConfig::from_env().unwrap_or_else(|err| {
        vlog::warn!(
            "Loading the config from the environment variables failed: {:?}",
            err
        );
        vlog::warn!("Using the hard-coded config, assuming it's the development run");
        LoadtestConfig::default()
    });

    let mut executor = Executor::new(config).await?;
    let final_resolution = executor.start().await;

    match final_resolution {
        LoadtestResult::TestPassed => {
            vlog::info!("Test passed");
            Ok(())
        }
        LoadtestResult::TestFailed => {
            vlog::error!("Test failed");
            Err(anyhow::anyhow!("Test failed"))
        }
    }
}
