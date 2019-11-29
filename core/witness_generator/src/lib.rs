pub mod client;
// Built-in
use std::{error, net, time};
// External
use actix_web::{
    App,
    HttpServer,
    HttpRequest,
    HttpResponse,
    web,
    Responder,
};
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct ProverReq {
    pub name: String,
}

struct AppState {
    connection_pool: storage::ConnectionPool,
    prover_timeout: time::Duration
}

fn register(data: web::Data<AppState>, r: web::Json<ProverReq>) -> actix_web::Result<String> {
    if r.name == "" {
        return Err(actix_web::error::ErrorBadRequest("empty name"))
    }
    let storage = match data.connection_pool.access_storage(){
        Ok(s) => s,
        Err(e) => {
            return Err(actix_web::error::ErrorInternalServerError(e))
        },
    };
    let id = match storage.register_prover(&r.name) {
        Ok(id) => id,
        Err(e) => {
            return Err(actix_web::error::ErrorInternalServerError(e))
        }
    };
    Ok(id.to_string())
}

#[derive(Serialize, Deserialize)]
pub struct BlockToProveRes {
    pub prover_run_id: i32,
    pub block: i64,
}

fn block_to_prove(data: web::Data<AppState>, r: web::Json<ProverReq>) -> actix_web::Result<HttpResponse> {
    if r.name == "" {
        return Err(actix_web::error::ErrorBadRequest("empty name"))
    }
    let storage = match data.connection_pool.access_storage(){
        Ok(s) => s,
        Err(e) => {
            return Err(actix_web::error::ErrorInternalServerError(e))
        },
    };
    // TODO: handle errors
    let ret = storage.job_for_unverified_block(&r.name, data.prover_timeout).unwrap();
    if let Some(prover_run) = ret {
        return Ok(HttpResponse::Ok().json(BlockToProveRes{
            prover_run_id: prover_run.id,
            block: prover_run.block_number,
        }))
    }
    Ok(HttpResponse::Ok().json(BlockToProveRes{
        prover_run_id: 0,
        block: 0,
    }))
}

#[derive(Serialize, Deserialize)]
pub struct WorkingOnReq {
    pub prover_run_id: i32,
}

fn working_on(data: web::Data<AppState>, r: web::Json<WorkingOnReq>) -> actix_web::Result<()> {
    let storage = match data.connection_pool.access_storage(){
        Ok(s) => s,
        Err(e) => {
            return Err(actix_web::error::ErrorInternalServerError(e))
        },
    };
    // TODO: handle errors
    storage.record_prover_is_working(r.prover_run_id).unwrap();
    Ok(())
}

pub fn start_server(bind_to: &net::SocketAddr, prover_timeout: time::Duration) {
    // TODO: add logging
    HttpServer::new(move || {
        App::new()
            .wrap(actix_web::middleware::Logger::default())
            .data(AppState {
                connection_pool: storage::ConnectionPool::new(),
                prover_timeout,
            })
            .route("/register", web::post().to(register))
            .route("/block_to_prove", web::get().to(block_to_prove))
            .route("/working_on", web::post().to(working_on))
    })
    .bind(bind_to)
    .unwrap()
    .run()
    .unwrap();
}
