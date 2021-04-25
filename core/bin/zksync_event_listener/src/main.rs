use zksync_config::ZkSyncConfig;
use zksync_event_listener::run_event_server;

fn main() {
    let config = ZkSyncConfig::from_env();

    let mut sys = actix_web::rt::System::builder()
        .name("event-listener")
        .stop_on_panic(true)
        .build();

    sys.block_on(run_event_server(config));
}
