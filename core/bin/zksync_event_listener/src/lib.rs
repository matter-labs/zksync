//! The `zksync_event_listener` is a stand-alone server-application responsible for
//! fetching new events that happen in the zkSync network from the database
//! and streaming them to the connected WebSocket clients.

// Built-in uses
// Workspace uses
use zksync_config::ZkSyncConfig;
// External uses
use actix::prelude::*;
use actix_web::{web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
// Local uses
use listener::EventListener;
use monitor::ServerMonitor;
use subscriber::Subscriber;

pub mod listener;
pub mod messages;
pub mod monitor;
pub mod subscriber;

#[derive(Debug)]
struct AppState {
    server_monitor: Addr<ServerMonitor>,
}

async fn ws_index(
    req: HttpRequest,
    stream: web::Payload,
    data: web::Data<AppState>,
) -> Result<HttpResponse, Error> {
    ws::start(Subscriber::new(data.server_monitor.clone()), &req, stream)
}

pub async fn run_event_server(config: ZkSyncConfig) {
    let server_monitor = ServerMonitor::new().start();
    EventListener::new(server_monitor.clone(), &config)
        .await
        .unwrap()
        .start();

    let state = web::Data::new(AppState { server_monitor });

    HttpServer::new(move || {
        App::new()
            .app_data(state.clone())
            .wrap(vlog::actix_middleware())
            .route("/", web::get().to(ws_index))
    })
    .bind(config.event_listener.ws_bind_addr())
    .unwrap()
    .run()
    .await
    .unwrap();
}
