#![cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]

use std::sync::mpsc;
use crate::models::{TransferTx, PublicKey};
use super::state_keeper::StateProcessingRequest;

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
    tx_for_state: mpsc::Sender<StateProcessingRequest>,
}

fn handle_send_transaction(req: &HttpRequest<AppState>) -> Box<Future<Item = HttpResponse, Error = Error>> {
    let tx_for_tx = req.state().tx_for_tx.clone();
    let tx_for_state = req.state().tx_for_state.clone();
    req.json()
        .from_err() // convert all errors into `Error`
        .and_then(move |tx: TransferTx| {
        
            // TODO: the code below will block the current thread; switch to futures instead
            let (key_tx, key_rx) = mpsc::channel();
            let request = StateProcessingRequest::GetPubKey(tx.from, key_tx);
            tx_for_state.send(request);
            // now wait for state_keeper to return a result
            let pub_key: Option<PublicKey> = key_rx.recv().unwrap();
                        
            let accepted = pub_key.as_ref().map(|pk| tx.verify_sig(pk)).unwrap_or(false);
            if accepted {
                let mut tx = tx.clone();
                tx.cached_pub_key = pub_key;
                tx_for_tx.send(tx); // pass to mem_pool
            }
            let resp = TransactionResponse{
                accepted
            };
            Ok(HttpResponse::Ok().json(resp))
        })
        .responder()
}

pub fn run_api_server(tx_for_tx:    mpsc::Sender<TransferTx>, 
                      tx_for_state: mpsc::Sender<StateProcessingRequest>) {

    ::std::env::set_var("RUST_LOG", "actix_web=info");
    let sys = actix::System::new("ws-example");

    //move is necessary to give closure below ownership
    server::new(move || {
        App::with_state(AppState {
            tx_for_tx: tx_for_tx.clone(),
            tx_for_state: tx_for_state.clone(),
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