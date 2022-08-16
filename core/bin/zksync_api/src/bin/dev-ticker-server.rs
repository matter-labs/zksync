//! Ticker implementation for dev environment
//!
//! Implements coinmarketcap API for tokens deployed using `deploy-dev-erc20`
//! Prices are randomly distributed around base values estimated from real world prices.

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpRequest, HttpResponse, HttpServer, Result};
use bigdecimal::BigDecimal;
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{collections::HashMap, fs::read_to_string, path::Path};
use std::{convert::TryFrom, time::Duration};
use structopt::StructOpt;
use zksync_crypto::rand::{thread_rng, Rng};
use zksync_types::Address;

#[derive(Debug, Serialize, Deserialize)]
struct CoinMarketCapTokenQuery {
    symbol: String,
}

macro_rules! make_sloppy {
    ($f: ident) => {{
        |query, data| async {
            if thread_rng().gen_range(0, 100) < 5 {
                vlog::debug!("`{}` has been errored", stringify!($f));
                return Ok(HttpResponse::InternalServerError().finish());
            }

            let duration = match thread_rng().gen_range(0, 100) {
                0..=59 => Duration::from_millis(100),
                60..=69 => Duration::from_secs(5),
                _ => {
                    let ms = thread_rng().gen_range(100, 1000);
                    Duration::from_millis(ms)
                }
            };

            vlog::debug!(
                "`{}` has been delayed for {}ms",
                stringify!($f),
                duration.as_millis()
            );
            tokio::time::sleep(duration).await;

            let resp = $f(query, data).await;
            resp
        }
    }};
}

async fn handle_coinmarketcap_token_price_query(
    query: web::Query<CoinMarketCapTokenQuery>,
    _data: web::Data<Vec<TokenData>>,
) -> Result<HttpResponse> {
    let symbol = query.symbol.clone();
    let base_price = match symbol.as_str() {
        "ETH" => BigDecimal::from(200),
        "wBTC" => BigDecimal::from(9000),
        "BAT" => BigDecimal::try_from(0.2).unwrap(),
        // Even though these tokens have their base price equal to
        // the default one, we still keep them here so that in the future it would
        // be easier to change the default price without affecting the important tokens
        "DAI" => BigDecimal::from(1),
        "tGLM" => BigDecimal::from(1),
        "GLM" => BigDecimal::from(1),
        "RBTC" => BigDecimal::from(18000),
        "RIF" => BigDecimal::from(0.053533),
        _ => BigDecimal::from(1),
    };
    let random_multiplier = thread_rng().gen_range(0.9, 1.1);

    let price = base_price * BigDecimal::try_from(random_multiplier).unwrap();

    let last_updated = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let resp = json!({
        "data": {
            symbol: {
                "quote": {
                    "USD": {
                        "price": price.to_string(),
                        "last_updated": last_updated
                    }
                }
            }
        }
    });
    vlog::info!("1.0 {} = {} USD", query.symbol, price);
    Ok(HttpResponse::Ok().json(resp))
}

#[derive(Debug, Deserialize)]
struct Token {
    pub address: Address,
    pub decimals: u8,
    pub symbol: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct TokenData {
    id: String,
    symbol: String,
    name: String,
    platforms: HashMap<String, Address>,
}

fn load_tokens(path: impl AsRef<Path>) -> Vec<TokenData> {
    if let Ok(text) = read_to_string(path) {
        let tokens: Vec<Token> = serde_json::from_str(&text).unwrap();
        let tokens_data: Vec<TokenData> = tokens
            .into_iter()
            .map(|token| {
                let symbol = token.symbol.to_lowercase();
                let mut platforms = HashMap::new();
                platforms.insert(String::from("ethereum"), token.address);
                let id = match symbol.as_str() {
                    "eth" => String::from("ethereum"),
                    "wbtc" => String::from("wrapped-bitcoin"),
                    "bat" => String::from("basic-attention-token"),
                    "RBTC" => String::from("RSK-smart-bitcoin"),
                    "RIF" => String::from("RSK-infrastructure-framework"),
                    _ => symbol.clone(),
                };

                TokenData {
                    id,
                    symbol: symbol.clone(),
                    name: symbol,
                    platforms,
                }
            })
            .collect();
        tokens_data
    } else {
        Vec::new()
    }
}

async fn handle_coingecko_token_list(
    _req: HttpRequest,
    data: web::Data<Vec<TokenData>>,
) -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json((*data.into_inner()).clone()))
}

async fn handle_coingecko_token_price_query(
    req: HttpRequest,
    _data: web::Data<Vec<TokenData>>,
) -> Result<HttpResponse> {
    let coin_id = req.match_info().get("coin_id");
    let base_price = match coin_id {
        Some("ethereum") => BigDecimal::from(200),
        Some("wrapped-bitcoin") => BigDecimal::from(9000),
        Some("basic-attention-token") => BigDecimal::try_from(0.2).unwrap(),
        Some("RSK-smart-bitcoin") => BigDecimal::from(18000),
        Some("RSK-infrastructure-framework") => BigDecimal::from(0.04),
        _ => BigDecimal::from(1),
    };
    let random_multiplier = thread_rng().gen_range(0.9, 1.1);
    let price = base_price * BigDecimal::try_from(random_multiplier).unwrap();

    let last_updated = Utc::now().timestamp_millis();
    let resp = json!({
        "prices": [
            [last_updated, price],
        ]
    });
    vlog::info!("1.0 {:?} = {} USD", coin_id, price);
    Ok(HttpResponse::Ok().json(resp))
}

fn main_scope(sloppy_mode: bool) -> actix_web::Scope {
    let localhost_tokens = load_tokens(&"etc/tokens/localhost.json");
    let testnet_tokens = load_tokens(&"etc/tokens/testnet.json");
    let data: Vec<TokenData> = localhost_tokens
        .into_iter()
        .chain(testnet_tokens.into_iter())
        .collect();
    if sloppy_mode {
        web::scope("/")
            .app_data(web::Data::new(data))
            .route(
                "/cryptocurrency/quotes/latest",
                web::get().to(make_sloppy!(handle_coinmarketcap_token_price_query)),
            )
            .route(
                "/api/v3/coins/list",
                web::get().to(make_sloppy!(handle_coingecko_token_list)),
            )
            .route(
                "/api/v3/coins/{coin_id}/market_chart",
                web::get().to(make_sloppy!(handle_coingecko_token_price_query)),
            )
    } else {
        web::scope("/")
            .app_data(web::Data::new(data))
            .route(
                "/cryptocurrency/quotes/latest",
                web::get().to(handle_coinmarketcap_token_price_query),
            )
            .route(
                "/api/v3/coins/list",
                web::get().to(handle_coingecko_token_list),
            )
            .route(
                "/api/v3/coins/{coin_id}/market_chart",
                web::get().to(handle_coingecko_token_price_query),
            )
    }
}

/// Ticker implementation for dev environment
///
/// Implements coinmarketcap API for tokens deployed using `deploy-dev-erc20`
/// Prices are randomly distributed around base values estimated from real world prices.
#[derive(Debug, StructOpt, Clone, Copy)]
struct FeeTickerOpts {
    /// Activate "sloppy" mode.
    ///
    /// With the option, server will provide a random delay for requests
    /// (60% of 0.1 delay, 30% of 0.1 - 1.0 delay, 10% of 5 seconds delay),
    /// and will randomly return errors for 5% of requests.
    #[structopt(long)]
    sloppy: bool,
}

fn main() {
    vlog::init();

    let opts = FeeTickerOpts::from_args();
    if opts.sloppy {
        vlog::info!("Fee ticker server will run in a sloppy mode.");
    }

    let runtime = actix_rt::System::new();
    runtime.block_on(async move {
        HttpServer::new(move || {
            App::new()
                .wrap(Cors::default().allow_any_origin().max_age(3600))
                .wrap(middleware::Logger::default())
                .service(main_scope(opts.sloppy))
        })
        .bind("0.0.0.0:9876")
        .unwrap()
        .shutdown_timeout(1)
        .run()
        .await
        .expect("Server crashed");
    });
}
