#![cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]

use std::sync::mpsc;
use crate::models::TransferTx;

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
    tx_for_tx: mpsc::Sender<TransferTx>,
}

fn verify_sig(tx: &TransferTx) -> bool {
    tx.verify_sig()
}

fn handle_send_transaction(req: &HttpRequest<AppState>) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let tx_for_tx = req.state().tx_for_tx.clone();
    req.json()
        .from_err() // convert all errors into `Error`
        .and_then(move |tx: TransferTx| {
            // let tx = TransferTx {
            //     from: val.from,
            //     to: val.to,
            //     amount: val.amount,
            //     fee: 0,
            //     nonce: 0,
            //     good_until_block: 100,
            //     sig_r: "".to_owned(),
            //     sig_s: "".to_owned(),
            // };
            let accepted = verify_sig(&tx);
            if accepted {
                tx_for_tx.send(tx); // pass to mem_pool
            }
            let resp = TransactionResponse{
                accepted
            };
            Ok(HttpResponse::Ok().json(resp))
        })
        .responder()
}

pub fn run_api_server(tx_for_tx: mpsc::Sender<TransferTx>) {

    ::std::env::set_var("RUST_LOG", "actix_web=info");
    let sys = actix::System::new("ws-example");

    //move is necessary to give closure below ownership
    server::new(move || {
        App::with_state(AppState {
            tx_for_tx: tx_for_tx.clone()
        }.clone()) // <- create app with shared state
            // enable logger
            .middleware(middleware::Logger::default())
            // enable CORS
            .configure(|app| {
                Cors::for_app(app)
                    .send_wildcard()
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