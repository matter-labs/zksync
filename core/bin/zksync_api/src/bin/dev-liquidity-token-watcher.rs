//! Token watcher implementation for dev environment
//!
//! Implements Uniswap API

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpResponse, HttpServer, Result};
use bigdecimal::BigDecimal;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize, Deserialize)]
struct GrqaphqlQuery {
    query: String,
}

async fn handle_graphql(params: web::Json<GrqaphqlQuery>) -> Result<HttpResponse> {
    // Now, we support only one graphql query, we will add full
    let query_parser = Regex::new(r#"\{token\(id:\s"(?P<address>.*?)"\).*"#).expect("Right regexp");
    let caps = query_parser.captures(&params.query).unwrap();
    let address = &caps["address"];
    let volume = match address {
        "0x6b175474e89094c44da98b954eedeac495271d0f" => BigDecimal::from(30000),
        "0x38A2fDc11f526Ddd5a607C1F251C065f40fBF2f7" => BigDecimal::from(50),
        _ => BigDecimal::from(0),
    };

    let response = json!({
        "data": {
            "token": {
                "tradeVolumeUSD": volume.to_string()
            }
        },
    });
    Ok(HttpResponse::Ok().json(response))
}

fn main() {
    env_logger::init();

    let mut runtime = actix_rt::System::new("dev-liquidity-token-watcher");

    runtime.block_on(async {
        HttpServer::new(move || {
            App::new()
                .wrap(middleware::Logger::default())
                .wrap(Cors::new().send_wildcard().max_age(3600).finish())
                .route("/graphql", web::post().to(handle_graphql))
        })
        .bind("0.0.0.0:9975")
        .unwrap()
        .shutdown_timeout(1)
        .run()
        .await
        .expect("Server crashed");
    });
}
