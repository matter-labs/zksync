#![cfg_attr(feature = "cargo-clippy", allow(needless_pass_by_value))]

extern crate actix;
extern crate actix_web;

use std::thread;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc;

use actix_web::{middleware, server, App, HttpRequest, HttpResponse};

/// Application state
struct AppState {

    secret_keys: Arc<Mutex<usize>>,
}

/// simple handle
fn index(req: &HttpRequest<AppState>) -> HttpResponse {
    println!("{:?}", req);
    *(req.state().counter.lock().unwrap()) += 1;

    HttpResponse::Ok().body(format!("Num of requests: {}", req.state().counter.lock().unwrap()))
}

fn main() {
    // create channel to accept deserialized requests for new transacitons

    let (tx_for_transactions, rx_for_transactions) = mpsc::channel();

    // create a separate 


    ::std::env::set_var("RUST_LOG", "actix_web=info");
    let sys = actix::System::new("ws-example");

    let counter = Arc::new(Mutex::new(0));
    //move is necessary to give closure below ownership of counter
    server::new(move || {
        App::with_state(AppState{counter: counter.clone()}) // <- create app with shared state
            // enable logger
            .middleware(middleware::Logger::default())
            // register simple handler, handle all methods
            .resource("/", |r| r.f(index))
    }).bind("127.0.0.1:8080")
        .unwrap()
        .start();

    println!("Started http server: 127.0.0.1:8080");
    let _ = sys.run();
}