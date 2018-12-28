#![cfg_attr(feature = "cargo-clippy", allow(clippy::needless_pass_by_value))]

use std::sync::mpsc;
use plasma::models::{TransferTx, PublicKey, Account};
use super::models::StateProcessingRequest;

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
    http::StatusCode,
};

use futures::Future;

use std::env;
use dotenv;

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

#[derive(Debug, Serialize, Deserialize)]
struct AccountError {
    error: String,
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
            tx_for_state.send(request).expect("queue must work");
            // now wait for state_keeper to return a result
            let pub_key: Option<PublicKey> = key_rx.recv().unwrap();
            
            let accepted = pub_key.as_ref().map(|pk| tx.verify_sig(pk) ).unwrap_or(false);
            if accepted {
                let mut tx = tx.clone();
                tx.cached_pub_key = pub_key;
                tx_for_tx.send(tx).expect("queue must work"); // pass to mem_pool
            }
            let resp = TransactionResponse{
                accepted
            };
            Ok(HttpResponse::Ok().json(resp))
        })
        .responder()
}

use actix_web::Result as ActixResult;

fn handle_get_state(req: &HttpRequest<AppState>) -> ActixResult<HttpResponse> {
    let tx_for_tx = req.state().tx_for_tx.clone();
    let tx_for_state = req.state().tx_for_state.clone();
    let account_id_string = req.match_info().get("id");
    if account_id_string.is_none() {
        return Ok(HttpResponse::Ok().json(AccountError{error:"invalid parameters".to_string()}));
    }
    let account_id = account_id_string.unwrap().parse::<u32>();
    if account_id.is_err(){
        return Ok(HttpResponse::Ok().json(AccountError{error:"invalid account_id".to_string()}));
    }
    let (acc_tx, acc_rx) = mpsc::channel();
    let request = StateProcessingRequest::GetLatestState(account_id.unwrap(), acc_tx);
    tx_for_state.send(request).expect("queue must work");
    let account_info: Option<Account> = acc_rx.recv().unwrap();
    if account_info.is_none() {
        return Ok(HttpResponse::Ok().json(AccountError{error:"non-existing account".to_string()}));
    }

    Ok(HttpResponse::Ok().json(account_info.unwrap()))
}

pub fn start_api_server(tx_for_tx:    mpsc::Sender<TransferTx>, 
                      tx_for_state: mpsc::Sender<StateProcessingRequest>) {
    
    dotenv().ok();

    let address = env::var("BIND_TO").unwrap_or("127.0.0.1");
    let port = env::var("PORT").unwrap_or("8080");


    std::thread::Builder::new().name("api_server".to_string()).spawn(move || {
        ::std::env::set_var("RUST_LOG", "actix_web=info");
        let sys = actix::System::new("ws-example");
        let server_config = format!("{}:{}", address, port);

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
                        .resource("/account/{id}", |r| {
                            r.method(Method::POST).f(|_| HttpResponse::Ok());
                            r.method(Method::OPTIONS).f(|_| HttpResponse::Ok());
                            r.method(Method::GET).f(handle_get_state);
                        })
                        .register()
                })
        }).bind(&server_config)
        .unwrap()
        .shutdown_timeout(1)
        .start();

        println!("Started http server: {}", server_config);
        sys.run();
    });
}