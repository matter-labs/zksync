#![cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]

use std::sync::mpsc;
use crate::models::tx::TxUnpacked;

use actix_web::{
    middleware, 
    server, 
    App, 
    AsyncResponder, 
    Error, 
    HttpMessage,
    HttpRequest, 
    HttpResponse, 
    middleware::cors::Cors,
    http::Method,
};

use futures::Future;

#[derive(Debug, Serialize, Deserialize)]
struct TransactionRequest {
    from: u32,
    to: u32,
    amount: u128
}

#[derive(Debug, Serialize, Deserialize)]
struct TransactionResponse {
    accepted: bool,
}

// singleton to keep info about channels required for Http server
#[derive(Clone)]
pub struct AppState {
    state_keeper_tx: mpsc::Sender<(TxUnpacked, mpsc::Sender<bool>)>,
}

pub fn handle_send_transaction(req: &HttpRequest<AppState>) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let state_tx = req.state().state_keeper_tx.clone();
    req.json()
        .from_err()  // convert all errors into `Error`
        .and_then(move |val: TransactionRequest| {
            let (tx, rx) = mpsc::channel::<bool>();
            let info = TxUnpacked {
                from: val.from,
                to: val.to,
                amount: val.amount,
                fee: 0,
                nonce: 0,
                good_until_block: 100,
                sig_r: "".to_owned(),
                sig_s: "".to_owned(),
            };
            state_tx.send((info, tx.clone()));
            let result = rx.recv();
            let resp = TransactionResponse{
                accepted: result.unwrap()
            };
            Ok(HttpResponse::Ok().json(resp))  // <- send response
        })
        .responder()
}

pub fn run_api_server(tx_for_transactions: mpsc::Sender<(TxUnpacked, mpsc::Sender<bool>)>) {

    ::std::env::set_var("RUST_LOG", "actix_web=info");
    let sys = actix::System::new("ws-example");

    //move is necessary to give closure below ownership
    server::new(move || {
        App::with_state(AppState {
            state_keeper_tx: tx_for_transactions.clone()
        }.clone()) // <- create app with shared state
            // enable logger
            .middleware(middleware::Logger::default())
            // enable CORS
            .configure(|app| {
                Cors::for_app(app)
                    // .allowed_origin("*")
                    .send_wildcard()
                    // .allowed_methods(vec!["GET", "POST", "OPTIONS"])
                    // .allowed_headers(vec![header::AUTHORIZATION, header::ACCEPT])
                    // .allowed_header(header::CONTENT_TYPE)
                    .max_age(3600)
                    .resource("/send", |r| {
                        r.method(Method::POST).f(handle_send_transaction);
                        r.method(Method::OPTIONS).f(|_| HttpResponse::Ok());
                        r.method(Method::GET).f(|_| HttpResponse::Ok());
                    })
                    .register()
            })
    }).bind("127.0.0.1:8080")
    .unwrap()
    .shutdown_timeout(1)
    .start();

    println!("Started http server: 127.0.0.1:8080");
    let _ = sys.run();
}