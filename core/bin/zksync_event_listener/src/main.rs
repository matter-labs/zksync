use zksync_config::ZkSyncConfig;
use zksync_event_listener::run_event_server;

fn main() {
    let _sentry_guard = vlog::init();
    let config = ZkSyncConfig::from_env();

    // TODO: `stop_on_panic` has no effect cause of tokio implementation.
    // Instead, the server should shutdown itself in case of an error. (ZKS-654).
    let mut sys = actix_web::rt::System::builder()
        .name("event-listener")
        .stop_on_panic(true)
        .build();

    sys.block_on(run_event_server(config));
}
