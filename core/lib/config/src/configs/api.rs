/// External uses
use serde::Deserialize;
/// Built-in uses
use std::net::SocketAddr;
// Workspace uses
use zksync_types::AccountId;
// Local uses
use crate::envy_load;

/// API configuration.
#[derive(Debug, Deserialize, Clone, PartialEq)]
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
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Common {
    // Size of LRU caches for requests
    pub caches_size: usize,
    // Determines the required minimum account age for `ForcedExit` operation to be allowed.
    // Type of value is seconds.
    pub forced_exit_minimum_account_age_secs: u64,
    /// List of account IDs that do not have to pay fees for operations.
    pub fee_free_accounts: Vec<AccountId>,
    pub enforce_pubkey_change_fee: bool,

    pub max_number_of_transactions_per_batch: u64,
    pub max_number_of_authors_per_batch: u64,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct AdminApi {
    /// Port to which the API server is listening.
    pub port: u16,
    /// URL to access API server.
    pub url: String,
    /// Secret used to generate access token (JWT).
    pub secret_auth: String,
}

impl AdminApi {
    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.port)
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ProverApi {
    /// Port to which the API server is listening.
    pub port: u16,
    /// URL to access API server.
    pub url: String,
    /// Secret used to generate access token (JWT).
    pub secret_auth: String,
}

impl ProverApi {
    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.port)
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct PrivateApi {
    /// Port to which the API server is listening.
    pub port: u16,
    /// URL to access API server.
    pub url: String,
}

impl PrivateApi {
    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.port)
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct RestApi {
    /// Port to which the API server is listening.
    pub port: u16,
    /// URL to access API server.
    pub url: String,
}

impl RestApi {
    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.port)
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
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

impl JsonRpc {
    pub fn http_bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.http_port)
    }

    pub fn ws_bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.ws_port)
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Prometheus {
    /// Port to which the Prometheus exporter server is listening.
    pub port: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::configs::test_utils::set_env;
    use std::net::IpAddr;

    fn expected_config() -> ApiConfig {
        ApiConfig {
            common: Common {
                caches_size: 10_000,
                forced_exit_minimum_account_age_secs: 0,
                enforce_pubkey_change_fee: true,
                max_number_of_transactions_per_batch: 200,
                max_number_of_authors_per_batch: 10,
                fee_free_accounts: vec![AccountId(4078), AccountId(387)],
            },
            admin: AdminApi {
                port: 8080,
                url: "http://127.0.0.1:8080".into(),
                secret_auth: "sample".into(),
            },
            rest: RestApi {
                port: 3001,
                url: "http://127.0.0.1:3001".into(),
            },
            json_rpc: JsonRpc {
                http_port: 3030,
                http_url: "http://127.0.0.1:3030".into(),
                ws_port: 3031,
                ws_url: "ws://127.0.0.1:3031".into(),
            },
            private: PrivateApi {
                port: 8090,
                url: "http://127.0.0.1:8090".into(),
            },
            prover: ProverApi {
                port: 8088,
                url: "http://127.0.0.1:8088".into(),
                secret_auth: "sample".into(),
            },
            prometheus: Prometheus { port: 3312 },
        }
    }

    #[test]
    fn from_env() {
        let config = r#"
API_COMMON_CACHES_SIZE="10000"
API_COMMON_FORCED_EXIT_MINIMUM_ACCOUNT_AGE_SECS="0"
API_COMMON_FEE_FREE_ACCOUNTS=4078,387
API_COMMON_ENFORCE_PUBKEY_CHANGE_FEE=true
API_COMMON_MAX_NUMBER_OF_TRANSACTIONS_PER_BATCH=200
API_COMMON_MAX_NUMBER_OF_AUTHORS_PER_BATCH=10
API_ADMIN_PORT="8080"
API_ADMIN_URL="http://127.0.0.1:8080"
API_ADMIN_SECRET_AUTH="sample"
API_REST_PORT="3001"
API_REST_URL="http://127.0.0.1:3001"
API_JSON_RPC_HTTP_PORT="3030"
API_JSON_RPC_HTTP_URL="http://127.0.0.1:3030"
API_JSON_RPC_WS_PORT="3031"
API_JSON_RPC_WS_URL="ws://127.0.0.1:3031"
API_PRIVATE_PORT="8090"
API_PRIVATE_URL="http://127.0.0.1:8090"
API_PROVER_PORT="8088"
API_PROVER_URL="http://127.0.0.1:8088"
API_PROVER_SECRET_AUTH="sample"
API_PROMETHEUS_PORT="3312"
        "#;
        set_env(config);

        let actual = ApiConfig::from_env();
        assert_eq!(actual, expected_config());
    }

    /// Checks the correctness of the config helper methods.
    #[test]
    fn methods() {
        let config = expected_config();
        let bind_broadcast_addr: IpAddr = "0.0.0.0".parse().unwrap();

        assert_eq!(
            config.admin.bind_addr(),
            SocketAddr::new(bind_broadcast_addr, config.admin.port)
        );
        assert_eq!(
            config.prover.bind_addr(),
            SocketAddr::new(bind_broadcast_addr, config.prover.port)
        );
        assert_eq!(
            config.rest.bind_addr(),
            SocketAddr::new(bind_broadcast_addr, config.rest.port)
        );
        assert_eq!(
            config.private.bind_addr(),
            SocketAddr::new(bind_broadcast_addr, config.private.port)
        );
        assert_eq!(
            config.json_rpc.http_bind_addr(),
            SocketAddr::new(bind_broadcast_addr, config.json_rpc.http_port)
        );
    }
}
