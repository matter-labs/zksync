/// External uses
use serde::Deserialize;
// Local uses
use crate::envy_load;

/// API configuration.
#[derive(Debug, Deserialize)]
pub struct ApiConfig {
    /// Common configuration options for the API.
    pub common: Common,
    /// Configuration options for the Admin API server.
    pub admin: AdminApi,
    /// Configuration options for the REST API server.
    pub rest: RestApi,
    /// Configuration options for the JSON RPC servers.
    pub json_rpc: JsonRpc,
    /// Configuration options for the private core API.
    pub private: PrivateApi,
    /// Configuration options for the prover server.
    pub prover: ProverApi,
    /// Configuration options for the Prometheus exporter.
    pub prometheus: Prometheus,
}

impl ApiConfig {
    pub fn from_env() -> Self {
        Self {
            common: envy_load!("common", "API_COMMON_"),
            admin: envy_load!("admin", "API_ADMIN_"),
            rest: envy_load!("rest", "API_REST_"),
            json_rpc: envy_load!("json_rpc", "API_JSON_RPC_"),
            private: envy_load!("private", "API_PRIVATE_"),
            prover: envy_load!("prover", "API_PROVER_"),
            prometheus: envy_load!("prometheus", "API_PROMETHEUS_"),
        }
    }
}

// Common configuration options for the API
#[derive(Debug, Deserialize)]
pub struct Common {
    // Size of LRU caches for requests
    pub caches_size: usize,
    // Determines the required minimum account age for `ForcedExit` operation to be allowed.
    // Type of value is seconds.
    pub forced_exit_minimum_account_age_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct AdminApi {
    /// Port to which the API server is listening.
    pub port: u16,
    /// URL to access API server.
    pub url: String,
    /// Secret used to generate access token (JWT).
    pub secret_auth: String,
}

#[derive(Debug, Deserialize)]
pub struct ProverApi {
    /// Port to which the API server is listening.
    pub port: u16,
    /// URL to access API server.
    pub url: String,
    /// Secret used to generate access token (JWT).
    pub secret_auth: String,
}

#[derive(Debug, Deserialize)]
pub struct PrivateApi {
    /// Port to which the API server is listening.
    pub port: u16,
    /// URL to access API server.
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct RestApi {
    /// Port to which the API server is listening.
    pub port: u16,
    /// URL to access API server.
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct JsonRpc {
    /// Port to which the HTTP RPC server is listening.
    pub http_port: u16,
    /// URL to access HTTP RPC server.
    pub http_url: String,
    /// Port to which the WebSocket RPC server is listening.
    pub ws_port: u16,
    /// URL to access WebSocket RPC server.
    pub ws_url: String,
}

#[derive(Debug, Deserialize)]
pub struct Prometheus {
    /// Port to which the Prometheus exporter server is listening.
    pub port: u16,
}
