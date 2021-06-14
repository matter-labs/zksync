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
use std::{convert::TryFrom, time::Duration};
use structopt::StructOpt;
use zksync_crypto::rand::{thread_rng, Rng};

#[derive(Debug, Serialize, Deserialize)]
struct CoinMarketCapTokenQuery {
    symbol: String,
}

macro_rules! make_sloppy {
    ($f: ident) => {{
        |query| async {
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
            tokio::time::delay_for(duration).await;

            let resp = $f(query).await;
            resp
        }
    }};
}

async fn handle_coinmarketcap_token_price_query(
    query: web::Query<CoinMarketCapTokenQuery>,
) -> Result<HttpResponse> {
    let symbol = query.symbol.clone();
    let base_price = match symbol.as_str() {
        "ETH" => BigDecimal::from(200),
        "wBTC" => BigDecimal::from(9000),
        "BAT" => BigDecimal::try_from(0.2).unwrap(),
        "DAI" => BigDecimal::from(1),
        "tGLM" => BigDecimal::from(1),
        "GLM" => BigDecimal::from(1),
        _ => BigDecimal::from(0),
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

async fn handle_coingecko_token_list(_req: HttpRequest) -> Result<HttpResponse> {
    // Some tokens may appear more than one time because they have different addresses on testnets.
    let resp = json!([
        {"id": "ethereum", "symbol": "eth", "name": "Ethereum", "platforms": {}},
        {"id": "dai", "symbol": "dai", "name": "Dai", "platforms": {"ethereum": "0x7eBAb6CBe1AAfc22c1877FEaa1D552b80CA91A09"}},
        {"id": "dai", "symbol": "dai", "name": "Dai", "platforms": {"ethereum": "0x351714Df444b8213f2C46aaA28829Fc5a8921304"}},
        {"id": "tglm", "symbol": "tglm", "name": "Golem", "platforms": {"ethereum": "0xd94e3DC39d4Cad1DAd634e7eb585A57A19dC7EFE"}},
        {"id": "usdc", "symbol": "usdc", "name": "usdc", "platforms": {"ethereum": "0xeb8f08a975Ab53E34D8a0330E0D34de942C95926"}},
        {"id": "usdt", "symbol": "usdt", "name": "usdt", "platforms": {"ethereum": "0x3B00Ef435fA4FcFF5C209a37d1f3dcff37c705aD"}},
        {"id": "tusd", "symbol": "tusd", "name": "tusd", "platforms": {"ethereum": "0x6856eC11F56267e3326f536D0e9F36eC7f7D1498"}},
        {"id": "tusd", "symbol": "tusd", "name": "tusd", "platforms": {"ethereum": "0xd2255612F9b045e9c81244bB874aBb413Ca139a3"}},
        {"id": "link", "symbol": "link", "name": "link", "platforms": {"ethereum": "0x4da8d0795830f75BE471F072a034d42c369B5d0A"}},
        {"id": "link", "symbol": "link", "name": "link", "platforms": {"ethereum": "0x793f38AE147852C37071684CdffC1FF7c87f7d07"}},
        {"id": "ht", "symbol": "ht", "name": "ht", "platforms": {"ethereum": "0x14700Cae8B2943bad34C70bB76AE27ECF5bC5013"}},
        {"id": "omg", "symbol": "omg", "name": "omg", "platforms": {"ethereum": "0x2B203de02AD6109521e09985b3aF9B8c62541Cd6"}},
        {"id": "trb", "symbol": "trb", "name": "trb", "platforms": {"ethereum": "0x2655F3a9eEB7F960be83098457144813ffaD07a4"}},
        {"id": "zrx", "symbol": "zrx", "name": "zrx", "platforms": {"ethereum": "0xC865bCBe4b6eF4B58a790052f2B51B4f06f586aC"}},
        {"id": "zrx", "symbol": "zrx", "name": "zrx", "platforms": {"ethereum": "0xDB7F2B9f6a0cB35FE5D236e5ed871D3aD4184290"}},
        {"id": "rep", "symbol": "rep", "name": "rep", "platforms": {"ethereum": "0x9Cac8508b9ff26501439590a24893D80e7E84D21"}},
        {"id": "storj", "symbol": "storj", "name": "storj", "platforms": {"ethereum": "0x8098165d982765097E4aa17138816e5b95f9fDb5"}},
        {"id": "nexo", "symbol": "nexo", "name": "nexo", "platforms": {"ethereum": "0x02d01f0835B7FDfa5d801A8f5f74c37F2BB1aE6a"}},
        {"id": "mco", "symbol": "mco", "name": "mco", "platforms": {"ethereum": "0xd93adDB2921b8061B697C2Ab055979BbEFE2B7AC"}},
        {"id": "knc", "symbol": "knc", "name": "knc", "platforms": {"ethereum": "0x290EBa6EC56EcC9fF81C72E8eccc77D2c2BF63eB"}},
        {"id": "lamb", "symbol": "lamb", "name": "lamb", "platforms": {"ethereum": "0x9ecec4d48Efdd96aE377aF3AB868f99De865CfF8"}},
        {"id": "basic-attention-token", "symbol": "bat", "name": "Basic Attention Token", "platforms": {"ethereum": "0xD2084eA2AE4bBE1424E4fe3CDE25B713632fb988"}},
        {"id": "basic-attention-token", "symbol": "bat", "name": "Basic Attention Token", "platforms": {"ethereum": "0x657aE665459c37483221C6a0c145a2DC197bD210"}},
        {"id": "basic-attention-token", "symbol": "bat", "name": "Basic Attention Token", "platforms": {"ethereum": "0x1B46bd2FC40030B6959A2d407f7D16f66aFaDD52"}},
        {"id": "wrapped-bitcoin", "symbol": "wbtc", "name": "Wrapped Bitcoin", "platforms": {"ethereum": "0x3bdFbbFDCF051C6EC5a741CC0fDe89e30Ff2F824"}},
    ]);

    Ok(HttpResponse::Ok().json(resp))
}

async fn handle_coingecko_token_price_query(req: HttpRequest) -> Result<HttpResponse> {
    let coin_id = req.match_info().get("coin_id");
    let base_price = match coin_id {
        Some("ethereum") => BigDecimal::from(200),
        Some("wrapped-bitcoin") => BigDecimal::from(9000),
        Some("basic-attention-token") => BigDecimal::try_from(0.2).unwrap(),
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
    if sloppy_mode {
        web::scope("/")
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

    let mut runtime = actix_rt::System::new("dev-ticker");
    runtime.block_on(async move {
        HttpServer::new(move || {
            App::new()
                .wrap(middleware::Logger::default())
                .wrap(Cors::new().send_wildcard().max_age(3600).finish())
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
