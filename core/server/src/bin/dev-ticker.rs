//! Ticker implementation for dev environment

use actix_cors::Cors;
use actix_web::{middleware, web, App, HttpResponse, HttpServer, Result};
use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug, Serialize, Deserialize)]
struct TokenQuery {
    symbol: String,
}

fn handle_block_explorer_search(
    // data: web::Data<AppState>,
    query: web::Query<TokenQuery>,
) -> Result<HttpResponse> {
    let symbol = query.symbol.clone();
    let price = "1.0".to_string();
    let last_updated = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
    let resp = json!({
        "data": {
            symbol: {
                "quote": {
                    "USD": {
                        "price": price,
                        "last_updated": last_updated
                    }
                }
            }
        }
    });
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
                    .route(web::get().to(handle_block_explorer_search)),
            )
    })
    .bind("0.0.0.0:9876")
    .unwrap()
    .shutdown_timeout(1)
    .start();

    runtime.run().unwrap_or_default();
}
