// Built-in uses
use std::collections::HashMap;

// External uses
use futures::{FutureExt, StreamExt};
use jsonrpc_core::Params;
use jsonrpc_http_server::{RequestMiddleware, RequestMiddlewareAction};

use super::types::RequestMetadata;

const CLOUDFLARE_CONNECTING_IP_HEADER: &str = "CF-Connecting-IP";

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
    ip: String,
) -> jsonrpc_core::MethodCall {
    // Methods, which should have the information about the ip appended to them
    let mut methods_with_ip: HashMap<String, MethodWithIpDescription> = HashMap::new();

    // Unfortunately at this moment the compiler from the CI does not support creating HashMap from iterator/array
    methods_with_ip.insert("tx_submit".to_owned(), MethodWithIpDescription::new(1, 4));
    methods_with_ip.insert(
        "submit_txs_batch".to_owned(),
        MethodWithIpDescription::new(1, 3),
    );
    methods_with_ip.insert("get_tx_fee".to_owned(), MethodWithIpDescription::new(3, 4));
    methods_with_ip.insert(
        "get_txs_batch_fee_in_wei".to_owned(),
        MethodWithIpDescription::new(3, 4),
    );

    let description = methods_with_ip.get(&call.method);
    let description = if let Some(desc) = description {
        desc
    } else {
        return call;
    };

    let mut new_call = call.clone();

    // We add ip only to array of parameters
    if let Params::Array(mut params) = call.params {
        // The query is definitely wrong. We may proceed and the server will handle it normally
        if params.len() > description.maximum_params || params.len() < description.minimum_params {
            return new_call;
        }

        // If the length is equsl to the maximum amount of the
        // maximum_params, then the user tried to override ip
        if params.len() == description.maximum_params {
            params.pop();
        }

        // Fill optional params with null
        while params.len() < description.maximum_params - 1 {
            params.push(serde_json::Value::Null);
        }

        let metadata = RequestMetadata { ip };
        let metadata = serde_json::to_value(metadata).unwrap();

        params.push(metadata);

        new_call.params = Params::Array(params);
        new_call
    } else {
        call
    }
}

/// Given the HTTP body of the JSON-RPC request and the IP of the user, inserts the information about it
/// in the call (if needed) and returns the bytes of the new body
async fn insert_ip(body: hyper::Body, ip: String) -> hyper::Result<Vec<u8>> {
    let body_stream: Vec<_> = body.collect().await;
    let body_bytes: hyper::Result<Vec<_>> = body_stream.into_iter().collect();

    // The `?` is here to let Rust resolve body_bytes as a vector of Bytes structs
    let mut body_bytes: Vec<u8> = body_bytes?
        .into_iter()
        .map(|b| b.into_iter().collect::<Vec<u8>>())
        .flatten()
        .collect();

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

        let remote_ip = match parts.headers.get(CLOUDFLARE_CONNECTING_IP_HEADER) {
            Some(ip) => ip.to_str(),
            None => {
                return RequestMiddlewareAction::Proceed {
                    should_continue_on_invalid_cors: false,
                    request: hyper::Request::from_parts(parts, body),
                }
            }
        };
        let remote_ip = if let Err(e) = remote_ip {
            vlog::warn!("Failed to parse CF-Connecting-IP header. Reason: {}", e);
            return RequestMiddlewareAction::Proceed {
                should_continue_on_invalid_cors: false,
                request: hyper::Request::from_parts(parts, body),
            };
        } else {
            remote_ip.unwrap()
        };

        let body_bytes = insert_ip(body, remote_ip.to_owned()).into_stream();
        let body = hyper::Body::wrap_stream(body_bytes);

        RequestMiddlewareAction::Proceed {
            should_continue_on_invalid_cors: false,
            request: hyper::Request::from_parts(parts, body),
        }
    }
}
