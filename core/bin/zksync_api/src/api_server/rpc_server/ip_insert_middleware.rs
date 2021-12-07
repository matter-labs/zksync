// Built-in uses
use std::collections::HashMap;

// External uses
use futures::{FutureExt, StreamExt};
use jsonrpc_core::Params;
use jsonrpc_http_server::{RequestMiddleware, RequestMiddlewareAction};

use super::types::RequestMetadata;

const CLOUDFLARE_CONNECTING_IP_HEADER: &str = "CF-Connecting-IP";
const METADATA_PARAM_NAME: &str = "meta";

///
/// Unfortunately, the JSON-RPC library does not natively support retrieving any information about the HTTP request,
///
/// But since the logic of subsidies relies on IP of the sender, we need to somehow extract the ip of the user from `CF-Connecting-IP`
/// header of HTTP request. This header IP inserted by Cloudflare and users can never set it by themselves.
///
/// IpInsertMiddleWare is the middleware that gets the value of the `CF-Connecting-IP` header of the HTTP request and appends it as the last
/// parameter of the JSON-RPC call.  
pub struct IpInsertMiddleWare {}

/// Struct which is used to describe the minimum number of parameters and the maximum number of parameters for a single JSON-RPC method
struct MethodWithIpDescription {
    minimum_params: usize,
    // the last one is always the ip parameter
    maximum_params: usize,
}

impl MethodWithIpDescription {
    pub fn new(minimum_params: usize, maximum_params: usize) -> Self {
        MethodWithIpDescription {
            minimum_params,
            maximum_params,
        }
    }
}

/// Get the original JSON-RPC MethodCall object and the IP of the user.
/// If the method does not need the information about the IP of the user, simply returns the supplied call.
/// If the method should have information about the IP appended to its parameters, it returns the new call
/// which is identical to the supplied one, but with the IP appended.
fn get_call_with_ip_if_needed(
    call: jsonrpc_core::MethodCall,
    ip: Option<String>,
) -> jsonrpc_core::MethodCall {
    // Methods, which should have the information about the ip appended to them
    let mut methods_with_ip: HashMap<&'static str, MethodWithIpDescription> = HashMap::new();

    // Unfortunately at this moment the compiler from the CI does not support creating HashMap from iterator/array
    methods_with_ip.insert("tx_submit", MethodWithIpDescription::new(1, 4));
    methods_with_ip.insert("submit_txs_batch", MethodWithIpDescription::new(1, 3));
    methods_with_ip.insert("get_tx_fee", MethodWithIpDescription::new(3, 4));
    methods_with_ip.insert(
        "get_txs_batch_fee_in_wei",
        MethodWithIpDescription::new(3, 4),
    );

    let description = methods_with_ip.get(call.method.as_str());
    let description = if let Some(desc) = description {
        desc
    } else {
        return call;
    };

    let metadata = ip.map(|ip| {
        let metadata = RequestMetadata { ip };
        serde_json::to_value(metadata).unwrap()
    });

    let mut new_call = call.clone();

    match call.params {
        Params::Array(mut params) => {
            // The query is definitely wrong. We may proceed and the server will handle it normally
            if params.len() > description.maximum_params
                || params.len() < description.minimum_params
            {
                return new_call;
            }

            // If the length is equsl to the maximum amount of the
            // maximum_params, then the user tried to override the metadata
            if params.len() == description.maximum_params {
                params.pop();
            }

            // Fill optional params with null
            while params.len() < description.maximum_params - 1 {
                params.push(serde_json::Value::Null);
            }

            if let Some(metadata) = metadata {
                params.push(metadata);
                new_call.params = Params::Array(params);
            }

            new_call
        }
        Params::Map(mut params_map) => {
            if let Some(metadata) = metadata {
                params_map.insert(METADATA_PARAM_NAME.to_owned(), metadata);
            } else {
                // Just in case the user tried to override the value in the map
                params_map.remove(METADATA_PARAM_NAME);
            }
            new_call.params = Params::Map(params_map);

            new_call
        }
        _ => call,
    }
}

/// Given the HTTP body of the JSON-RPC request and the IP of the user, inserts the information about it
/// in the call (if needed) and returns the bytes of the new body.
/// If the IP supplied is None, the method makes sure that the user could not pass the IP
async fn insert_ip_if_needed(body: hyper::Body, ip: Option<String>) -> hyper::Result<Vec<u8>> {
    let body_stream: Vec<_> = body.collect().await;

    let mut body_bytes = vec![];
    for bytes in body_stream {
        body_bytes.extend(bytes?.into_iter());
    }

    let body_str = String::from_utf8(body_bytes.clone());

    if let Ok(s) = body_str {
        let call: std::result::Result<jsonrpc_core::MethodCall, _> = serde_json::from_str(&s);
        if let Ok(call) = call {
            let new_call = get_call_with_ip_if_needed(call, ip);
            let new_body_bytes = serde_json::to_string(&new_call);
            if let Ok(s) = new_body_bytes {
                body_bytes = s.as_bytes().to_owned();
            }
        };
    }

    Ok(body_bytes)
}

impl RequestMiddleware for IpInsertMiddleWare {
    fn on_request(&self, request: hyper::Request<hyper::Body>) -> RequestMiddlewareAction {
        let (parts, body) = request.into_parts();

        let cloudflare_ip = parts
            .headers
            .get(CLOUDFLARE_CONNECTING_IP_HEADER)
            .map(|ip| ip.to_str().map(|s| s.to_owned()));

        let proceed = move |ip: Option<String>| {
            let body_bytes = insert_ip_if_needed(body, ip).into_stream();
            let body = hyper::Body::wrap_stream(body_bytes);
            RequestMiddlewareAction::Proceed {
                should_continue_on_invalid_cors: false,
                request: hyper::Request::from_parts(parts, body),
            }
        };

        match cloudflare_ip {
            None => {
                // We still need to check that the user didn't try to pass the metadata
                proceed(None)
            }
            Some(Err(e)) => {
                vlog::warn!("Failed to parse CF-Connecting-IP header. Reason: {}", e);
                // We still need to check that the user didn't try to pass the metadata
                proceed(None)
            }
            Some(Ok(ip)) => proceed(Some(ip)),
        }
    }
}
