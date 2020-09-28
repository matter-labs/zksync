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
use zksync_crypto::rand::{thread_rng, Rng};

#[derive(Debug, Serialize, Deserialize)]
struct CoinMarketCapTokenQuery {
    symbol: String,
}

async fn handle_coinmarketcap_token_price_query(
    query: web::Query<CoinMarketCapTokenQuery>,
) -> Result<HttpResponse> {
    let symbol = query.symbol.clone();
    let base_price = match symbol.as_str() {
        "ETH" => BigDecimal::from(200),
        "wBTC" => BigDecimal::from(9000),
        "BAT" => BigDecimal::from(0.2),
        "DAI" => BigDecimal::from(1),
        _ => BigDecimal::from(0),
    };
    let random_multiplier = thread_rng().gen_range(0.9, 1.1);

    let price = base_price * BigDecimal::from(random_multiplier);

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
    log::info!("1.0 {} = {} USD", query.symbol, price);
    Ok(HttpResponse::Ok().json(resp))
}

async fn handle_coingecko_token_list(_req: HttpRequest) -> Result<HttpResponse> {
    let resp = json!([
        {"id": "ethereum", "symbol": "eth", "name": "Ethereum"},
        {"id": "dai", "symbol":"dai", "name": "Dai"},
        {"id": "basic-attention-token", "symbol": "bat", "name": "Basic Attention Token"},
        {"id": "wrapped-bitcoin", "symbol": "wbtc", "name": "Wrapped Bitcoin"},
    ]);

    Ok(HttpResponse::Ok().json(resp))
}

async fn handle_coingecko_token_price_query(req: HttpRequest) -> Result<HttpResponse> {
    let coin_id = req.match_info().get("coin_id");
    let base_price = match coin_id {
        Some("ethereum") => BigDecimal::from(200),
        Some("wrapped-bitcoin") => BigDecimal::from(9000),
        Some("basic-attention-token") => BigDecimal::from(0.2),
        Some("dai") => BigDecimal::from(1),
        _ => BigDecimal::from(0),
    };
    let random_multiplier = thread_rng().gen_range(0.9, 1.1);
    let price = base_price * BigDecimal::from(random_multiplier);

    let last_updated = Utc::now().timestamp_millis();
    let resp = json!({
        "prices": [
            [last_updated, price],
        ]
    });
    log::info!("1.0 {:?} = {} USD", coin_id, price);
    Ok(HttpResponse::Ok().json(resp))
}

fn main() {
    env_logger::init();

    let mut runtime = actix_rt::System::new("dev-ticker");

    runtime.block_on(async {
        HttpServer::new(move || {
            App::new()
                .wrap(middleware::Logger::default())
                .wrap(Cors::new().send_wildcard().max_age(3600).finish())
                .service(
                    web::scope("/")
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
                        ),
                )
        })
        .bind("0.0.0.0:9876")
        .unwrap()
        .shutdown_timeout(1)
        .run()
        .await
        .expect("Server crashed");
    });
}
