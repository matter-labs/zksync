use zksync_config::ZkSyncConfig;
use zksync_core::gateway_watcher::GatewayWatcher;

#[tokio::main]
async fn main() {
    vlog::init();

    GatewayWatcher::from_config(&ZkSyncConfig::from_env())
        .run()
        .await;
}
