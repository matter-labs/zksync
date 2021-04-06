// Built-in uses
use std::collections::HashMap;

// External uses
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

// Workspace uses
use zksync_types::network::Network;

// Local uses

pub mod block;
pub mod config;
pub mod fee;
pub mod token;
pub mod transaction;

const API_V02_SCOPE: &str = "/api/v0.2/";

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ApiVersion {
    V02,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultStatus {
    Success,
    Error,
}
#[derive(Debug, Deserialize, Serialize)]
pub struct Request {
    pub network: Network,
    pub api_version: ApiVersion,
    pub resource: String,
    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    pub args: HashMap<String, String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Response {
    pub request: Request,
    pub status: ResultStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
}
