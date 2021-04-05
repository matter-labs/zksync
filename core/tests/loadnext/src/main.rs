use loadnext::{config::LoadtestConfig, executor::Executor, report_collector::FinalResolution};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    vlog::init();

    let config = LoadtestConfig::from_env().unwrap_or_else(|err| {
        vlog::warn!(
            "Loading the config from the environment variables failed: {:?}",
            err
        );
        vlog::warn!("Using the hard-coded config, assuming it's the development run");

        LoadtestConfig {
            zksync_rpc_addr: "http://127.0.0.1:3030".into(),
            web3_url: "http://127.0.0.1:8545".into(),
            master_wallet_pk: "74d8b3a188f7260f67698eb44da07397a298df5427df681ef68c45b34b61f998"
                .into(),
            accounts_amount: 20,
            operations_per_account: 40,
            main_token: "DAI".into(),
        }
    });

    let mut executor = Executor::new(config).await;
    let final_resolution = executor.start().await;

    match final_resolution {
        FinalResolution::TestPassed => {
            vlog::info!("Test passed");
            Ok(())
        }
        FinalResolution::TestFailed => {
            vlog::error!("Test failed");
            Err(anyhow::anyhow!("Test failed"))
        }
    }
}
