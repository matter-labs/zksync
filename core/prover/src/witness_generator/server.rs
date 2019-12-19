mod pool;

// Built-in
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::thread;
use std::{error, net, time};
// External
use actix_web::web::delete;
use actix_web::{web, App, HttpRequest, HttpResponse, HttpServer, Responder};
use ff::{Field, PrimeField};
use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use log::{error, info};
use pairing::bn256::Bn256;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
// Workspace deps
use circuit::account::AccountWitness;
use circuit::operation::{
    Operation, OperationArguments, OperationBranch, OperationBranchWitness, SignatureData,
};
use models::merkle_tree::PedersenHasher;
use models::node::tx::PackedPublicKey;
use models::node::{Engine, Fr};

struct AppState {
    connection_pool: storage::ConnectionPool,
    preparing_data_pool: Arc<RwLock<pool::ProversDataPool>>,
    prover_timeout: time::Duration,
}

#[derive(Serialize, Deserialize)]
pub struct ProverReq {
    pub name: String,
}

fn register(data: web::Data<AppState>, r: web::Json<ProverReq>) -> actix_web::Result<String> {
    info!("register request for prover with name: {}", r.name);
    if r.name == "" {
        return Err(actix_web::error::ErrorBadRequest("empty name"));
    }
    let storage = match data.connection_pool.access_storage() {
        Ok(s) => s,
        Err(e) => return Err(actix_web::error::ErrorInternalServerError(e)),
    };
    let id = match storage.register_prover(&r.name) {
        Ok(id) => id,
        Err(e) => return Err(actix_web::error::ErrorInternalServerError(e)),
    };
    Ok(id.to_string())
}

#[derive(Serialize, Deserialize)]
pub struct BlockToProveRes {
    pub prover_run_id: i32,
    pub block: i64,
}

fn block_to_prove(
    data: web::Data<AppState>,
    r: web::Json<ProverReq>,
) -> actix_web::Result<HttpResponse> {
    info!("request block to prove from worker: {}", r.name);
    if r.name == "" {
        return Err(actix_web::error::ErrorBadRequest("empty name"));
    }
    let storage = match data.connection_pool.access_storage() {
        Ok(s) => s,
        Err(e) => return Err(actix_web::error::ErrorInternalServerError(e)),
    };
    match storage.next_unverified_commit(&r.name, data.prover_timeout) {
        Ok(ret) => {
            if let Some(prover_run) = ret {
                return Ok(HttpResponse::Ok().json(BlockToProveRes {
                    prover_run_id: prover_run.id,
                    block: prover_run.block_number,
                }));
            }
            Ok(HttpResponse::Ok().json(BlockToProveRes {
                prover_run_id: 0,
                block: 0,
            }))
        }
        Err(e) => {
            error!("could not get next unverified commit operation: {}", e);
            Err(actix_web::error::ErrorInternalServerError(
                "storage layer error",
            ))
        }
    }
}

fn prover_data(
    data: web::Data<AppState>,
    block: web::Json<i64>,
) -> actix_web::Result<HttpResponse> {
    info!("requesting prover_data for block {}", *block);
    let data_pool = data.preparing_data_pool.read().unwrap();
    Ok(HttpResponse::Ok().json(data_pool.get(*block)))
}

#[derive(Serialize, Deserialize)]
pub struct WorkingOnReq {
    pub prover_run_id: i32,
}

fn working_on(data: web::Data<AppState>, r: web::Json<WorkingOnReq>) -> actix_web::Result<()> {
    info!(
        "working on request for prover_run with id: {}",
        r.prover_run_id
    );
    let storage = match data.connection_pool.access_storage() {
        Ok(s) => s,
        Err(e) => return Err(actix_web::error::ErrorInternalServerError(e)),
    };
    match storage.record_prover_is_working(r.prover_run_id) {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("failed to record prover work in progress request: {}", e);
            Err(actix_web::error::ErrorInternalServerError(
                "storage layer error",
            ))
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct PublishReq {
    pub block: u32,
    pub proof: models::EncodedProof,
}

fn publish(data: web::Data<AppState>, r: web::Json<PublishReq>) -> actix_web::Result<()> {
    info!("publish of a proof for block: {}", r.block);
    let storage = match data.connection_pool.access_storage() {
        Ok(s) => s,
        Err(e) => return Err(actix_web::error::ErrorInternalServerError(e)),
    };
    match storage.store_proof(r.block, &r.proof) {
        Ok(_) => {
            let mut data_pool = data.preparing_data_pool.write().unwrap();
            data_pool.clean_up(r.block as i64);
            Ok(())
        }
        Err(e) => {
            error!("failed to store received proof: {}", e);
            Err(actix_web::error::ErrorInternalServerError(
                "storage layer error",
            ))
        }
    }
}

pub fn start_server(
    bind_to: &net::SocketAddr,
    prover_timeout: time::Duration,
    rounds_interval: time::Duration,
) {
    let data_pool = Arc::new(RwLock::new(pool::ProversDataPool::new()));
    // TODO: graceful thread exit?
    let data_pool_copy = Arc::clone(&data_pool);
    thread::spawn(move || {
        let conn_pool = storage::ConnectionPool::new();
        pool::maintain(conn_pool, data_pool_copy, rounds_interval);
    });
    let ret = HttpServer::new(move || {
        App::new()
            .wrap(actix_web::middleware::Logger::default())
            .data(AppState {
                // TODO: receive conn pool?
                connection_pool: storage::ConnectionPool::new(),
                preparing_data_pool: Arc::clone(&data_pool),
                prover_timeout,
            })
            .route("/register", web::post().to(register))
            .route("/block_to_prove", web::get().to(block_to_prove))
            .route("/working_on", web::post().to(working_on))
            .route("/prover_data", web::get().to(prover_data))
    })
    .bind(bind_to)
    .unwrap()
    .run();
}
