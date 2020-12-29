//! Token watcher implementation for dev environment
//!
//! Implements Uniswap API for token which are deployed in localhost network
use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpResponse, HttpServer, Result};
use bigdecimal::BigDecimal;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use zksync_api::fee_ticker::validator::watcher::{
    GraphqlResponse, GraphqlTokenResponse, TokenResponse,
};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct TokenData {
    address: String,
    name: String,
    volume: BigDecimal,
}

type Tokens = HashMap<String, TokenData>;

#[derive(Debug, Serialize, Deserialize)]
struct GrqaphqlQuery {
    query: String,
}

async fn handle_graphql(
    params: web::Json<GrqaphqlQuery>,
    tokens: web::Data<Tokens>,
) -> Result<HttpResponse> {
    // Now, we support only one graphql query, we will add full
    let query_parser = Regex::new(r#"\{token\(id:\s"(?P<address>.*?)"\).*"#).expect("Right regexp");
    let caps = query_parser.captures(&params.query).unwrap();
    let address = &caps["address"].to_ascii_lowercase();
    let volume = if let Some(token) = tokens.get(address) {
        token.volume.clone()
    } else {
        BigDecimal::from(0)
    };
    let response = GraphqlResponse {
        data: GraphqlTokenResponse {
            token: Some(TokenResponse {
                trade_volume_usd: volume.to_string(),
            }),
        },
    };
    Ok(HttpResponse::Ok().json(response))
}

fn load_tokens(path: &Path) -> Tokens {
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);

    let values: Vec<Value> = serde_json::from_reader(reader).unwrap();
    let mut tokens: Tokens = Default::default();
    for value in values {
        if let Value::Object(value) = value {
            let address = value["address"].as_str().unwrap().to_ascii_lowercase();
            tokens.insert(
                address.clone(),
                TokenData {
                    address,
                    name: value["name"].to_string(),
                    volume: BigDecimal::from(500),
                },
            );
        };
    }
    let phnx_token = TokenData {
        address: "0x6b175474e89094c44da98b954eedeac495271d0f".to_string(),

        name: "PHNX".to_string(),
        volume: BigDecimal::from(30),
    };
    tokens.insert(phnx_token.address.clone(), phnx_token);
    tokens
}

fn main() {
    env_logger::init();

    let mut runtime = actix_rt::System::new("dev-liquidity-token-watcher");

    let tokens = load_tokens(Path::new(&"etc/tokens/localhost.json".to_string()));
    runtime.block_on(async {
        HttpServer::new(move || {
            App::new()
                .wrap(middleware::Logger::default())
                .wrap(Cors::new().send_wildcard().max_age(3600).finish())
                .data(tokens.clone())
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
