use num::{rational::Ratio, BigUint};
/// External uses
use serde::Deserialize;
/// Built-in uses
use std::net::SocketAddr;
use std::time::Duration;
use zksync_utils::scaled_u64_to_ratio;
// Workspace uses
use zksync_types::AccountId;
// Local uses
use crate::envy_load;

/// API configuration.
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ApiConfig {
    /// Common configuration options for the API.
    pub common: CommonApiConfig,
    /// Configuration options for the Admin API server.
    pub admin: AdminApiConfig,
    /// Configuration options for the REST API server.
    pub rest: RestApiConfig,
    /// Configuration options for the JSON RPC servers.
    pub json_rpc: JsonRpcConfig,
    /// Configuration options for the web3 JSON RPC server.
    pub web3: Web3Config,
    /// Configuration options for the private core API.
    pub private: PrivateApiConfig,
    /// Configuration options for the prover server.
    pub prover: ProverApiConfig,
    /// Configuration options for the Prometheus exporter.
    pub prometheus: PrometheusConfig,
    pub token_config: TokenConfig,
}

impl ApiConfig {
    pub fn from_env() -> Self {
        Self {
            common: envy_load!("common", "API_COMMON_"),
            admin: envy_load!("admin", "API_ADMIN_"),
            rest: envy_load!("rest", "API_REST_"),
            json_rpc: envy_load!("json_rpc", "API_JSON_RPC_"),
            web3: envy_load!("web3", "API_WEB3_"),
            private: envy_load!("private", "API_PRIVATE_"),
            prover: envy_load!("prover", "API_PROVER_"),
            prometheus: envy_load!("prometheus", "API_PROMETHEUS_"),
            token_config: envy_load!("token", "API_TOKEN_"),
        }
    }
}

impl CommonApiConfig {
    pub fn max_subsidy_usd(&self) -> Ratio<BigUint> {
        scaled_u64_to_ratio(self.max_subsidy_usd_scaled)
    }

    pub fn from_env() -> Self {
        envy_load!("common", "API_COMMON_")
    }
}

impl AdminApiConfig {
    pub fn from_env() -> Self {
        envy_load!("admin", "API_ADMIN_")
    }
}

impl RestApiConfig {
    pub fn from_env() -> Self {
        envy_load!("rest", "API_REST_")
    }
}

impl JsonRpcConfig {
    pub fn from_env() -> Self {
        envy_load!("json_rpc", "API_JSON_RPC_")
    }
}

impl Web3Config {
    pub fn from_env() -> Self {
        envy_load!("web3", "API_WEB3_")
    }
}

impl PrivateApiConfig {
    pub fn from_env() -> Self {
        envy_load!("private", "API_PRIVATE_")
    }
}
impl ProverApiConfig {
    pub fn from_env() -> Self {
        envy_load!("prover", "API_PROVER_")
    }
}

impl PrometheusConfig {
    pub fn from_env() -> Self {
        envy_load!("prometheus", "API_PROMETHEUS_")
    }
}

// Common configuration options for the API
#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct CommonApiConfig {
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

    /// The IPs which have their CPK (CREATE2) subsidized
    pub subsidized_ips: Vec<String>,

    /// Maxiumum subsidized amout for current subsidy type scaled by SUBSIDY_USD_AMOUNTS_SCALE
    pub max_subsidy_usd_scaled: u64,

    /// The name of current subsidy. It is needed to conveniently fetch historical data regarding subsidies for different partners
    pub subsidy_name: String,
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct TokenConfig {
    /// The interval of updating tokens from database
    pub invalidate_token_cache_period_sec: u64,
}

impl TokenConfig {
    pub fn from_env() -> TokenConfig {
        envy_load!("token", "API_TOKEN_")
    }

    pub fn invalidate_token_cache_period(&self) -> Duration {
        Duration::from_secs(self.invalidate_token_cache_period_sec)
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct AdminApiConfig {
    /// Port to which the API server is listening.
    pub port: u16,
    /// URL to access API server.
    pub url: String,
    /// Secret used to generate access token (JWT).
    pub secret_auth: String,
}

impl AdminApiConfig {
    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.port)
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct ProverApiConfig {
    /// Port to which the API server is listening.
    pub port: u16,
    /// URL to access API server.
    pub url: String,
    /// Secret used to generate access token (JWT).
    pub secret_auth: String,
}

impl ProverApiConfig {
    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.port)
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct PrivateApiConfig {
    /// Port to which the API server is listening.
    pub port: u16,
    /// URL to access API server.
    pub url: String,
}

impl PrivateApiConfig {
    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.port)
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct RestApiConfig {
    /// Port to which the API server is listening.
    pub port: u16,
    /// URL to access API server.
    pub url: String,
}

impl RestApiConfig {
    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.port)
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct JsonRpcConfig {
    /// Port to which the HTTP RPC server is listening.
    pub http_port: u16,
    /// URL to access HTTP RPC server.
    pub http_url: String,
    /// Port to which the WebSocket RPC server is listening.
    pub ws_port: u16,
    /// URL to access WebSocket RPC server.
    pub ws_url: String,
}

impl JsonRpcConfig {
    pub fn http_bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.http_port)
    }

    pub fn ws_bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.ws_port)
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct Web3Config {
    /// Port to which the web3 JSON RPC server is listening.
    pub port: u16,
    /// URL to access web3 JSON RPC server.
    pub url: String,
    /// Max difference between blocks in `eth_getLogs` method.
    pub max_block_range: u32,
    pub chain_id: u64,
}

impl Web3Config {
    pub fn bind_addr(&self) -> SocketAddr {
        SocketAddr::new("0.0.0.0".parse().unwrap(), self.port)
    }
}

#[derive(Debug, Deserialize, Clone, PartialEq)]
pub struct PrometheusConfig {
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
            common: CommonApiConfig {
                caches_size: 10_000,
                forced_exit_minimum_account_age_secs: 0,
                enforce_pubkey_change_fee: true,
                max_number_of_transactions_per_batch: 200,
                max_number_of_authors_per_batch: 10,
                fee_free_accounts: vec![AccountId(4078), AccountId(387)],
                subsidized_ips: vec!["127.0.0.1".to_owned()],
                max_subsidy_usd_scaled: 20000,
                subsidy_name: String::from("PartnerName"),
            },
            admin: AdminApiConfig {
                port: 8080,
                url: "http://127.0.0.1:8080".into(),
                secret_auth: "sample".into(),
            },
            rest: RestApiConfig {
                port: 3001,
                url: "http://127.0.0.1:3001".into(),
            },
            json_rpc: JsonRpcConfig {
                http_port: 3030,
                http_url: "http://127.0.0.1:3030".into(),
                ws_port: 3031,
                ws_url: "ws://127.0.0.1:3031".into(),
            },
            web3: Web3Config {
                port: 3002,
                url: "http://127.0.0.1:3002".into(),
                max_block_range: 10,
                chain_id: 240,
            },
            private: PrivateApiConfig {
                port: 8090,
                url: "http://127.0.0.1:8090".into(),
            },
            prover: ProverApiConfig {
                port: 8088,
                url: "http://127.0.0.1:8088".into(),
                secret_auth: "sample".into(),
            },
            prometheus: PrometheusConfig { port: 3312 },
            token_config: TokenConfig {
                invalidate_token_cache_period_sec: 10,
            },
        }
    }

    #[test]
    fn from_env() {
        let config = r#"
API_COMMON_CACHES_SIZE="10000"
API_COMMON_FORCED_EXIT_MINIMUM_ACCOUNT_AGE_SECS="0"
API_COMMON_FEE_FREE_ACCOUNTS=4078,387
API_COMMON_ENFORCE_PUBKEY_CHANGE_FEE=true
API_COMMON_SUBSIDIZED_IPS="127.0.0.1"
API_COMMON_MAX_SUBSIDY_USD_SCALED=20000
API_COMMON_SUBSIDY_NAME=PartnerName
API_COMMON_MAX_NUMBER_OF_TRANSACTIONS_PER_BATCH=200
API_COMMON_MAX_NUMBER_OF_AUTHORS_PER_BATCH=10
API_TOKEN_INVALIDATE_TOKEN_CACHE_PERIOD_SEC="10"
API_ADMIN_PORT="8080"
API_ADMIN_URL="http://127.0.0.1:8080"
API_ADMIN_SECRET_AUTH="sample"
API_REST_PORT="3001"
API_REST_URL="http://127.0.0.1:3001"
API_JSON_RPC_HTTP_PORT="3030"
API_JSON_RPC_HTTP_URL="http://127.0.0.1:3030"
API_JSON_RPC_WS_PORT="3031"
API_JSON_RPC_WS_URL="ws://127.0.0.1:3031"
API_WEB3_PORT="3002"
API_WEB3_URL="http://127.0.0.1:3002"
API_WEB3_CHAIN_ID="240"
API_WEB3_MAX_BLOCK_RANGE="10"
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
        assert_eq!(
            config.web3.bind_addr(),
            SocketAddr::new(bind_broadcast_addr, config.web3.port)
        );
    }
}
