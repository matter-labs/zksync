//! Loggers for rest.rs, and HTTP/WS RPC.
//!
//! All logs print headers in HEADERS constant.
//! Rest logs also report request url, response duration in milliseconds,
//! and response code.
//!
//! To enable all logs, add to RUST_LOG env variable:
//! actix_web=info,server::api_server::loggers=trace
//!
//! You can also cherry-pick, e.g. print only logs for websocket handshake
//! and not for http rpc or api_server:
//! server::api_server::loggers::ws_rpc=trace,

/// Headers to be printed in every request
const HEADERS: [&str; 5] = [
    "cf-connecting-ip",
    "cf-ipcountry",
    "user-agent",
    "cf-request-id",
    "cf-ray",
];

pub mod rest {
    use super::HEADERS;
    use itertools::Itertools;

    pub fn get_logger_format() -> String {
        let headers_formatted = HEADERS
            .iter()
            .map(|&h| format!("{}=\"%{{{}}}i\"", h, h))
            .join(" ");

        format!(
            "request=\"%r\" \
            {}
            resp-code=\"%s\" \
            resp-duration=\"%Dms\"",
            headers_formatted,
        )
    }
}

pub mod http_rpc {
    use super::HEADERS;
    use itertools::Itertools;
    use jsonrpc_http_server::{
        hyper::{http::HeaderValue, Body, Request},
        RequestMiddlewareAction,
    };
    use log::Level;

    pub fn request_middleware(request: Request<Body>) -> RequestMiddlewareAction {
        if log::log_enabled!(Level::Info) {
            let get_header = |header| {
                request
                    .headers()
                    .get(header)
                    .map(HeaderValue::to_str)
                    .transpose()
                    .unwrap_or(Some("parse error"))
                    .unwrap_or("-")
            };

            let headers_formatted = HEADERS
                .iter()
                .map(|&h| format!("{}: \"{}\"", h, get_header(h)))
                .join(", ");

            log::trace!("{}", headers_formatted,);
        }

        request.into()
    }
}

pub mod ws_rpc {
    use super::HEADERS;
    use itertools::Itertools;
    use jsonrpc_ws_server::ws::{Request, Response};
    use log::Level;
    use std::{collections::HashMap, ops::Deref};

    pub fn request_middleware(request: &Request) -> Option<Response> {
        if log::log_enabled!(Level::Info) {
            let mut headers = HashMap::with_capacity(request.headers().len());

            for (k, v) in request.headers() {
                let header_val = std::str::from_utf8(&v).unwrap_or("parse error");
                headers.insert(k.as_str(), header_val);
            }

            let get_header = |header| headers.get(header).map(Deref::deref).unwrap_or("-");

            let headers_formatted = HEADERS
                .iter()
                .map(|&h| format!("{}: \"{}\"", h, get_header(h)))
                .join(", ");

            log::trace!("{}", headers_formatted,);
        }

        None
    }
}
