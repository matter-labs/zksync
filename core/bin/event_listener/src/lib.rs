// Built-in uses
// Workspace uses
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

pub fn run_event_server() {
    let mut sys = actix_web::rt::System::new("event-server");

    sys.block_on(async {
        let server_monitor = ServerMonitor::new().start();
        EventListener::new(server_monitor.clone())
            .await
            .unwrap()
            .start();

        let state = web::Data::new(AppState { server_monitor });

        HttpServer::new(move || {
            App::new()
                .app_data(state.clone())
                .route("/ws/", web::get().to(ws_index))
        })
        .bind("127.0.0.1:9999")
        .unwrap()
        .run()
        .await
        .unwrap()
    });
}
