use zksync_core::run_core;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    run_core().await
}
