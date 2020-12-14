// Public re-exports
pub use self::api_config::ApiConfig;

/// Convenience macro that loads the structure from the environment variable given the prefix.
///
/// # Panics
///
/// Panics if the config cannot be loaded from the environment variables.
macro_rules! envy_load {
    ($name:expr, $prefix:expr) => {
        envy::prefixed($prefix)
            .from_env()
            .unwrap_or_else(|err| panic!("Cannot load config <{}>: {}", $name, err))
    };
}

/// Convenience macro that loads the structure from the TOML file given the path.
///
/// # Panics
///
/// Panics if the config cannot be loaded from the file.
macro_rules! toml_load {
    ($name:expr, $path:expr) => {{
        let file_contents = std::fs::read_to_string($path).unwrap_or_else(|err| {
            panic!(
                "Cannot load config <{}> from file <{}>: {}",
                $name, $path, err
            )
        });
        toml::from_str(&file_contents).unwrap_or_else(|err| {
            panic!(
                "Cannot parse config <{}> from file <{}>: {}",
                $name, $path, err
            )
        })
    }};
}

pub mod api_config {
    use serde::Deserialize;

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

        pub fn from_toml(path: &str) -> Self {
            toml_load!("api", path)
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
}

pub mod chain {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct ChainConfig {
        /// Proving / circuit data configuration.
        pub circuit: Circuit,
        /// L1 parameters configuration.
        pub eth: Eth,
        /// State keeper / block generating configuration.
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
            toml_load!("chain", path)
        }
    }

    #[derive(Debug, Deserialize)]
    pub struct Circuit {
        /// Path to the directory with the cryptographical keys. Relative to `$ZKSYNC_HOME`.
        pub key_dir: String,
        /// Actual supported block chunks sizes by verifier contract (determined by circuit size on setup boundaries).
        pub supported_block_chunks_sizes: Vec<usize>,
        /// Setup power needed to proof block of certain size (goes in the same order as the previous field,
        /// so both arrays can be `zip`ped together).
        pub supported_block_chunks_sizes_setup_powers: Vec<usize>,
        /// Depth of the Account Merkle tree.
        pub account_tree_depth: usize,
        /// Depth of the Balance Merkle tree.
        pub balance_tree_depth: usize,
    }

    #[derive(Debug, Deserialize)]
    pub struct Eth {
        /// Since withdraw is an expensive operation, we have to limit amount of
        /// withdrawals in one block to not exceed the gas limit in prover.
        /// If this threshold is reached, block will be immediately sealed and
        /// the remaining withdrawals will go to the next block.
        pub max_number_of_withdrawals_per_block: usize,
        /// Name of the used Ethereum network, e.g. `localhost` or `rinkeby`.
        pub eth_network: String,
    }

    #[derive(Debug, Deserialize)]
    pub struct StateKeeper {
        /// Block sizes to be generated by server. Has to contain only values set in the `supported_block_chunks_sizes`,
        /// otherwise block will never be proven. This list can contain not all the values though: e.g. for local
        /// development usually a couple of smallest block sizes is enough.
        pub block_chunk_sizes: Vec<usize>,
        /// Time between two miniblocks created by mempool / block_proposer.
        pub miniblock_iteration_interval: u64,
        /// Maximum amount of miniblock iterations before sealing the block.
        pub miniblock_iterations: u64,
        /// Maximum amount of miniblock iterations in case of block containing a fast withdrawal request.
        pub fast_block_miniblock_iterations: u64,
    }
}

pub mod contracts {
    // External uses
    use serde::Deserialize;
    // Workspace uses
    use zksync_types::{Address, H256};

    /// Data about deployed contracts.
    #[derive(Debug, Deserialize)]
    pub struct ContractsConfig {
        pub upgrade_gatekeeper: Address,
        pub governance_target: Address,
        pub verifier_target: Address,
        pub contract_target: Address,
        pub contract: Address,
        pub governance: Address,
        pub verifier: Address,
        pub deploy_factory: Address,
        pub genesis_tx_hash: H256,
    }

    impl ContractsConfig {
        pub fn from_env() -> Self {
            envy_load!("contracts", "CONTRACTS_")
        }

        pub fn from_toml(path: &str) -> Self {
            toml_load!("contracts", path)
        }
    }
}

pub mod db {
    // External uses
    use serde::Deserialize;

    /// Used database configuration.
    #[derive(Debug, Deserialize)]
    pub struct DBConfig {
        /// Amount of open connections to the database held by server in the pool.
        pub pool_size: usize,
        /// Database URL.
        pub url: String,
    }

    impl DBConfig {
        pub fn from_env() -> Self {
            envy_load!("db", "DB_")
        }

        pub fn from_toml(path: &str) -> Self {
            toml_load!("db", path)
        }
    }
}

pub mod eth_client {
    // External uses
    use serde::Deserialize;
    // Workspace uses

    /// Configuration for the Ethereum gateways.
    #[derive(Debug, Deserialize)]
    pub struct ETHClientConfig {
        /// Numeric identifier of the L1 network (e.g. `9` for localhost).
        pub chain_id: u64,
        /// How much do we want to increase gas price provided by the network?
        /// Normally it's 1, we use the network-provided price (and limit it with the gas adjuster in eth sender).
        /// However, it can be increased to speed up the transaction mining time.
        pub gas_price_factor: f64,
        /// Address of the Ethereum node API.
        pub web3_url: String,
    }

    impl ETHClientConfig {
        pub fn from_env() -> Self {
            envy_load!("eth_client", "ETH_CLIENT_")
        }

        pub fn from_toml(path: &str) -> Self {
            toml_load!("eth_client", path)
        }
    }
}

pub mod eth_sender {
    // External uses
    use serde::Deserialize;
    // Workspace uses
    use zksync_types::{Address, H256};

    /// Configuration for the Ethereum sender crate.
    #[derive(Debug, Deserialize)]
    pub struct ETHSenderConfig {
        /// Options related to the Ethereum sender directly.
        pub sender: Sender,
        /// Options related to the `gas_adjuster` submodule.
        pub gas_limit: GasLimit,
    }

    impl ETHSenderConfig {
        pub fn from_env() -> Self {
            Self {
                sender: envy_load!("eth_sender", "ETH_SENDER_SENDER_"),
                gas_limit: envy_load!("eth_sender.gas_limit", "ETH_SENDER_GAS_LIMIT_"),
            }
        }

        pub fn from_toml(path: &str) -> Self {
            toml_load!("eth_sender", path)
        }
    }

    #[derive(Debug, Deserialize)]
    pub struct Sender {
        /// Private key of the operator account.
        pub operator_private_key: H256,
        /// Address of the operator account.
        pub operator_commit_eth_addr: Address,
        /// mount of confirmations required to consider L1 transaction committed.
        pub wait_confirmations: u64,
        /// Amount of blocks we will wait before considering L1 transaction stuck.
        pub expected_wait_time_block: u64,
        /// Node polling period in seconds.
        pub tx_poll_period: u64,
        /// The maximum amount of simultaneously sent Ethereum transactions.
        pub max_txs_in_flight: u64,
        /// Whether sender should interact with L1 or not.
        pub is_enabled: bool,
    }

    #[derive(Debug, Deserialize)]
    pub struct GasLimit {
        /// Gas price limit to be used by GasAdjuster until the statistics data is gathered.
        pub default: u64,
        /// Interval between updates of the gas price limit (used by GasAdjuster) in seconds.
        pub update_interval: u64,
        /// Interval between adding the Ethereum node gas price to the GasAdjuster (in seconds).
        pub sample_interval: u64,
        /// Scale factor for gas price limit (used by GasAdjuster).
        pub scale_factor: f64,
    }
}

pub mod eth_watch {
    // External uses
    use serde::Deserialize;
    // Workspace uses

    /// Configuration for the Ethereum sender crate.
    #[derive(Debug, Deserialize)]
    pub struct ETHWatchConfig {
        /// Amount of confirmations for the priority operation to be processed.
        /// In production this should be a non-zero value because of block reverts.
        pub confirmations_for_eth_event: u64,
        /// How often we want to poll the Ethereum node.
        pub eth_node_poll_interval: u64,
    }

    impl ETHWatchConfig {
        pub fn from_env() -> Self {
            envy_load!("eth_watch", "ETH_WATCH_")
        }

        pub fn from_toml(path: &str) -> Self {
            toml_load!("eth_watch", path)
        }
    }
}
