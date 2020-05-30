//! Ticker implementation for dev environment
//!
//! Implements coinmarketcap API for tokens deployed using `deploy-dev-erc20`
//! Prices are randomly distributed around base values estimated from real world prices.

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpResponse, HttpServer, Result};
use bigdecimal::BigDecimal;
use chrono::{SecondsFormat, Utc};
use crypto_exports::rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize, Deserialize)]
struct TokenQuery {
    symbol: String,
}

fn handle_token_price_query(query: web::Query<TokenQuery>) -> Result<HttpResponse> {
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

fn main() {
    env_logger::init();

    let runtime = actix_rt::System::new("dev-ticker");

    HttpServer::new(move || {
        App::new()
            .wrap(middleware::Logger::default())
            .wrap(Cors::new().send_wildcard().max_age(3600))
            .service(
                web::resource("/cryptocurrency/quotes/latest")
                    .route(web::get().to(handle_token_price_query)),
            )
    })
    .bind("0.0.0.0:9876")
    .unwrap()
    .shutdown_timeout(1)
    .start();

    runtime.run().unwrap_or_default();
}
