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

fn load_tokens(path: &Path) -> Vec<(String, String)> {
    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);

    let values: Vec<Value> = serde_json::from_reader(reader).unwrap();
    let mut tokens = vec![];
    for value in values {
        if let Value::Object(value) = value {
            let address = value["address"].as_str().unwrap().to_ascii_lowercase();
            tokens.push((address, value["name"].to_string()))
        };
    }
    tokens
}
#[derive(Clone)]
struct VolumeStorage {
    whitelist_tokens: Tokens,
    blacklist_tokens: HashSet<String>,
    default_volume: BigDecimal,
    regime: Regime,
}

impl VolumeStorage {
    fn new(regime: Regime, default_volume: BigDecimal) -> Self {
        Self {
            whitelist_tokens: Default::default(),
            blacklist_tokens: Default::default(),
            default_volume,
            regime,
        }
    }

    fn with_whitelisted_tokens(mut self, tokens: Vec<(String, String)>) -> Self {
        let mut whitelist_tokens: Tokens = Default::default();
        for (address, name) in tokens {
            whitelist_tokens.insert(
                address.clone(),
                TokenData {
                    address,
                    name,
                    volume: self.default_volume.clone(),
                },
            );
        }
        self.whitelist_tokens = whitelist_tokens;
        self
    }

    fn with_blacklist_tokens(mut self, tokens: HashSet<String>) -> Self {
        self.blacklist_tokens = tokens;
        self
    }

    fn get_volume(&self, address: &str) -> BigDecimal {
        match self.regime {
            Regime::Blacklist => {
                if self.blacklist_tokens.get(address).is_some() {
                    BigDecimal::from(0)
                } else {
                    self.default_volume.clone()
                }
            }
            Regime::Whitelist => {
                let volume = if let Some(token) = self.whitelist_tokens.get(address) {
                    token.volume.clone()
                } else {
                    BigDecimal::from(0)
                };
                volume
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct GrqaphqlQuery {
    query: String,
}

async fn handle_graphql(
    params: web::Json<GrqaphqlQuery>,
    volume_storage: web::Data<VolumeStorage>,
) -> Result<HttpResponse> {
    // Now, we support only one graphql query, we will add full
    let query_parser = Regex::new(r#"\{token\(id:\s"(?P<address>.*?)"\).*"#).expect("Right regexp");
    let caps = query_parser.captures(&params.query).unwrap();
    let address = &caps["address"].to_ascii_lowercase();
    let volume = volume_storage.get_volume(address);
    let response = GraphqlResponse {
        data: GraphqlTokenResponse {
            token: Some(TokenResponse {
                trade_volume_usd: volume.to_string(),
            }),
        },
    };
    Ok(HttpResponse::Ok().json(response))
}

fn main() {
    env_logger::init();

    let mut runtime = actix_rt::System::new("dev-liquidity-token-watcher");

    let config = DevLiquidityTokenWatcherConfig::from_env();
    let whitelisted_tokens = load_tokens(Path::new(&"etc/tokens/localhost.json".to_string()));
    let storage = VolumeStorage::new(config.regime, config.default_volume.into())
        .with_whitelisted_tokens(whitelisted_tokens)
        .with_blacklist_tokens(config.blacklisted_tokens);
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
        let storage = VolumeStorage::new(Regime::Whitelist, 500.into())
            .with_whitelisted_tokens(vec![token.clone()]);
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
        let storage =
            VolumeStorage::new(Regime::Blacklist, 500.into()).with_blacklist_tokens(tokens);
        let volume = storage.get_volume(&token);
        assert_eq!(volume, 0.into());
        let volume = storage.get_volume("another_token");
        assert_eq!(volume, 500.into())
    }
}
