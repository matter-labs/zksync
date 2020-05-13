use itertools::Itertools;

const OTHER_HEADERS: [&str; 19] = [
    "user-agent",
    "cf-request-id",
    "host",
    "cf-ray",
    "cf-visitor",
    "content-length",
    "accept-encoding",
    "accept",
    "content-type",
    "referrer",
    "x-request-id",
    "x-real-ip",
    "x-forwarded-for",
    "x-forwarded-host",
    "x-forwarded-port",
    "x-forwarded-proto",
    "x-original-uri",
    "x-scheme",
    "x-original-forwarded-for",
];

pub mod rest {
    use super::*;

    pub fn get_logger_format() -> String {
        let other_headers_formatted = OTHER_HEADERS
            .iter()
            .map(|&h| format!("{}: \"%{{{}}}i\"", h, h))
            .join(", ");

        format!(
            "req {{ \"%r\" from %{{cf-connecting-ip}}i, %{{cf-ipcountry}}i }}, \
            resp {{ code: %s, bytes: %b, duration: %Dms }}, req headers {{ {} }}",
            other_headers_formatted
        )
    }
}

pub mod http_rpc {
    use super::*;
    use jsonrpc_http_server::hyper;
    use jsonrpc_http_server::hyper::http::HeaderValue;
    use jsonrpc_http_server::RequestMiddlewareAction;

    pub fn request_middleware(request: hyper::Request<hyper::Body>) -> RequestMiddlewareAction {
        let get_header = |header| {
            request
                .headers()
                .get(header)
                .map(HeaderValue::to_str)
                .transpose()
                .unwrap_or(Some("parse error"))
                .unwrap_or("-")
        };

        let other_headers_formatted = OTHER_HEADERS
            .iter()
            .map(|&h| format!("{}: \"{}\"", h, get_header(h)))
            .join(", ");

        info!(
            "req from {}, {}, req headers {{ {} }}",
            get_header("cf-connecting-ip"),
            get_header("cf-ipcountry"),
            other_headers_formatted,
        );

        request.into()
    }
}

pub mod ws_rpc {
    use super::*;
    use jsonrpc_ws_server::ws::Request;
    use jsonrpc_ws_server::ws::Response;
    use std::collections::HashMap;
    use std::ops::Deref;

    pub fn request_middleware(request: &Request) -> Option<Response> {
        let mut headers = HashMap::with_capacity(request.headers().len());

        for (k, v) in request.headers() {
            let header_val = std::str::from_utf8(&v).unwrap_or("parse error");
            headers.insert(k.as_str(), header_val);
        }

        let get_header = |header| headers.get(header).map(Deref::deref).unwrap_or("-");

        let other_headers_formatted = OTHER_HEADERS
            .iter()
            .map(|&h| format!("{}: \"{}\"", h, get_header(h)))
            .join(", ");

        info!(
            "handshake from {}, {}, other headers {{ {} }}",
            get_header("cf-connecting-ip"),
            get_header("cf-ipcountry"),
            other_headers_formatted,
        );

        None
    }
}
