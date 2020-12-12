// Public re-exports
pub use self::api_config::ApiConfig;

/// Convenience macro that loads the structure from the environment variable given the prefix.
macro_rules! envy_load {
    ($name:expr, $prefix:expr) => {
        envy::prefixed($prefix)
            .from_env()
            .unwrap_or_else(|err| panic!("Cannot load config <{}>: {}", $name, err))
    };
}

pub mod api_config {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct ApiConfig {
        pub common: Common,
        pub admin: AdminApi,
        pub rest: RestApi,
        pub json_rpc: JsonRpc,
        pub private: PrivateApi,
        pub prover: ProverApi,
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

        pub fn from_toml(path: &str) -> Self {
            let file_contents = std::fs::read_to_string(path)
                .unwrap_or_else(|err| panic!("Cannot load config file <{}>: {}", path, err));
            toml::from_str(&file_contents)
                .unwrap_or_else(|err| panic!("Cannot parse config file <{}>: {}", path, err))
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
        pub port: u16,
        pub url: String,
        pub secret_auth: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct ProverApi {
        pub port: u16,
        pub url: String,
        pub secret_auth: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct PrivateApi {
        pub port: u16,
        pub url: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct RestApi {
        pub port: u16,
        pub url: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct JsonRpc {
        pub http_port: u16,
        pub http_url: String,
        pub ws_port: u16,
        pub ws_url: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct Prometheus {
        pub port: u16,
    }
}

pub mod chain {
    use serde::Deserialize;
    #[derive(Debug, Deserialize)]
    pub struct ChainConfig {
        pub circuit: Circuit,
        pub eth: Eth,
        pub state_keeper: StateKeeper,
    }

    impl ChainConfig {
        pub fn from_env() -> Self {
            Self {
                circuit: envy_load!("circuit", "CHAIN_CIRCUIT_"),
                eth: envy_load!("eth", "CHAIN_ETH_"),
                state_keeper: envy_load!("state_keeper", "CHAIN_STATE_KEEPER_"),
            }
        }

        pub fn from_toml(path: &str) -> Self {
            let file_contents = std::fs::read_to_string(path)
                .unwrap_or_else(|err| panic!("Cannot load config file <{}>: {}", path, err));
            toml::from_str(&file_contents)
                .unwrap_or_else(|err| panic!("Cannot parse config file <{}>: {}", path, err))
        }
    }

    #[derive(Debug, Deserialize)]
    pub struct Circuit {
        pub key_dir: String,
        pub supported_block_chunks_sizes: Vec<usize>,
        pub supported_block_chunks_sizes_setup_powers: Vec<usize>,
        pub account_tree_depth: usize,
        pub balance_tree_depth: usize,
    }

    #[derive(Debug, Deserialize)]
    pub struct Eth {
        pub max_number_of_withdrawals_per_block: usize,
        pub eth_network: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct StateKeeper {
        pub block_chunk_sizes: Vec<usize>,
        pub miniblock_iteration_interval: u64,
        pub miniblock_iterations: u64,
        pub fast_block_miniblock_iterations: u64,
    }
}
