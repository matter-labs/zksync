use zksync_config::ZkSyncConfig;
use zksync_event_listener::run_event_server;

fn main() {
    let _vlog_guard = vlog::init();
    let config = ZkSyncConfig::from_env();

    let sys = actix_web::rt::System::new();

    sys.block_on(run_event_server(config));
}
