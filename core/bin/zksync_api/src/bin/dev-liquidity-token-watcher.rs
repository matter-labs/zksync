//! Token watcher implementation for dev environment
//!
//! Implements Uniswap API for token which are deployed in localhost network
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::BufReader,
    path::Path,
};

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpResponse, HttpServer, Result};
use bigdecimal::BigDecimal;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use zksync_api::fee_ticker::validator::watcher::{
    GraphqlResponse, GraphqlTokenResponse, TokenResponse,
};
use zksync_config::{configs::dev_liquidity_token_watcher::Regime, DevLiquidityTokenWatcherConfig};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct TokenData {
    address: String,
    name: String,
    volume: BigDecimal,
}

type Tokens = HashMap<String, TokenData>;

#[derive(Debug, Clone)]
enum VolumeStorage {
    Blacklist(HashSet<String>, BigDecimal),
    Whitelist(Tokens),
}

impl VolumeStorage {
    fn whitelisted_tokens(tokens: Vec<(String, String)>, default_volume: BigDecimal) -> Self {
        let whitelist_tokens: Tokens = tokens
            .into_iter()
            .map(|(address, name)| {
                (
                    address.clone(),
                    TokenData {
                        address,
                        name,
                        volume: default_volume.clone(),
                    },
                )
            })
            .collect();
        Self::Whitelist(whitelist_tokens)
    }

    fn blacklisted_tokens(tokens: HashSet<String>, default_volume: BigDecimal) -> Self {
        Self::Blacklist(tokens, default_volume)
    }

    fn get_volume(&self, address: &str) -> BigDecimal {
        match self {
            Self::Blacklist(tokens, default_volume) => {
                if tokens.get(address).is_some() {
                    BigDecimal::from(0)
                } else {
                    default_volume.clone()
                }
            }

            Self::Whitelist(tokens) => {
                let volume = if let Some(token) = tokens.get(address) {
                    token.volume.clone()
                } else {
                    BigDecimal::from(0)
                };
                volume
            }
        }
    }
}

fn load_tokens(path: impl AsRef<Path>) -> Vec<(String, String)> {
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);

    let values: Vec<HashMap<String, Value>> = serde_json::from_reader(reader).unwrap();
    let tokens: Vec<(String, String)> = values
        .into_iter()
        .map(|value| {
            let address = value["address"].as_str().unwrap().to_ascii_lowercase();
            (address, value["name"].to_string())
        })
        .collect();
    tokens
}

#[derive(Debug, Serialize, Deserialize)]
struct GrqaphqlQuery {
    query: String,
}

async fn handle_graphql(
    params: web::Json<GrqaphqlQuery>,
    volume_storage: web::Data<VolumeStorage>,
) -> Result<HttpResponse> {
    // TODO https://linear.app/matterlabs/issue/ZKS-413/support-full-version-of-graphql-for-tokenvalidator
    let query_parser = Regex::new(r#"\{token\(id:\s"(?P<address>.*?)"\).*"#).expect("Right regexp");
    let caps = query_parser.captures(&params.query).unwrap();
    let address = &caps["address"].to_ascii_lowercase();
    let volume = volume_storage.get_volume(address);
    let response = GraphqlResponse {
        data: GraphqlTokenResponse {
            token: Some(TokenResponse {
                untracked_volume_usd: volume.to_string(),
            }),
        },
    };
    Ok(HttpResponse::Ok().json(response))
}

fn main() {
    vlog::init();

    let mut runtime = actix_rt::System::new("dev-liquidity-token-watcher");
    let config = DevLiquidityTokenWatcherConfig::from_env();

    let storage = match config.regime {
        Regime::Blacklist => VolumeStorage::blacklisted_tokens(
            config.blacklisted_tokens,
            config.default_volume.into(),
        ),
        Regime::Whitelist => {
            let whitelisted_tokens = load_tokens(&"etc/tokens/localhost.json");
            VolumeStorage::whitelisted_tokens(whitelisted_tokens, config.default_volume.into())
        }
    };

    runtime.block_on(async {
        HttpServer::new(move || {
            App::new()
                .data(storage.clone())
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn get_volume_for_whitelisted() {
        let token = ("addr".to_string(), "name".to_string());
        let storage = VolumeStorage::whitelisted_tokens(vec![token.clone()], 500.into());

        let volume = storage.get_volume(&token.0);
        assert_eq!(volume, 500.into());
        let volume = storage.get_volume("wrong_addr");
        assert_eq!(volume, 0.into())
    }
    #[test]
    fn get_volume_for_blacklisted() {
        let token = "addr".to_string();
        let mut tokens = HashSet::new();
        tokens.insert(token.clone());
        let storage = VolumeStorage::blacklisted_tokens(tokens, 500.into());

        let volume = storage.get_volume(&token);
        assert_eq!(volume, 0.into());
        let volume = storage.get_volume("another_token");
        assert_eq!(volume, 500.into())
    }
}
