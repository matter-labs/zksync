// Built-in deps
use std::{collections::HashSet, env, net::SocketAddr, str::FromStr, time::Duration};
// External uses
use url::Url;
// Workspace uses
use zksync_types::{Address, H256};
use zksync_utils::{get_env, parse_env, parse_env_if_exists, parse_env_with};
// Local uses

pub mod test_config;

/// Makes address for bind from port.
fn addr_from_port(port: u16) -> SocketAddr {
    format!("0.0.0.0:{}", port)
        .parse::<SocketAddr>()
        .expect("Can't get address from port")
}

/// Configuration options for `eth_sender`.
#[derive(Debug, Clone)]
pub struct EthSenderOptions {
    pub expected_wait_time_block: u64,
    pub tx_poll_period: Duration,
    pub wait_confirmations: u64,
    pub max_txs_in_flight: u64,
    pub is_enabled: bool,
}

impl EthSenderOptions {
    /// Parses the `eth_sender` configuration options values from the environment variables.
    /// Panics if any of options is missing or has inappropriate value.
    pub fn from_env() -> Self {
        let tx_poll_period_secs: u64 = parse_env("ETH_TX_POLL_PERIOD");

        Self {
            expected_wait_time_block: parse_env("ETH_EXPECTED_WAIT_TIME_BLOCK"),
            tx_poll_period: Duration::new(tx_poll_period_secs, 0),
            wait_confirmations: parse_env("ETH_WAIT_CONFIRMATIONS"),
            max_txs_in_flight: parse_env("ETH_MAX_TXS_IN_FLIGHT"),
            is_enabled: parse_env("ETH_IS_ENABLED"),
        }
    }
}

/// Configuration options for `eth_client`.
#[derive(Debug, Clone)]
pub struct EthClientOptions {
    pub chain_id: u8,
    pub gas_price_factor: f64,
    pub operator_commit_eth_addr: Address,
    pub operator_private_key: Option<H256>,
    pub web3_url: String,
    pub contract_eth_addr: Address,
}

impl EthClientOptions {
    pub fn from_env() -> Self {
        Self {
            operator_commit_eth_addr: parse_env_with("OPERATOR_COMMIT_ETH_ADDRESS", |s| &s[2..]),
            operator_private_key: parse_env_if_exists("OPERATOR_PRIVATE_KEY"),
            chain_id: parse_env("CHAIN_ID"),
            gas_price_factor: parse_env("GAS_PRICE_FACTOR"),
            web3_url: get_env("WEB3_URL"),
            contract_eth_addr: parse_env_with("CONTRACT_ADDR", |s| &s[2..]),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProverOptions {
    pub secret_auth: String,
    pub prepare_data_interval: Duration,
    pub heartbeat_interval: Duration,
    pub cycle_wait: Duration,
    pub gone_timeout: Duration,
    pub prover_server_address: SocketAddr,
    pub idle_provers: u32,
    pub witness_generators: usize,
}

impl ProverOptions {
    /// Parses the configuration options values from the environment variables.
    /// Panics if any of options is missing or has inappropriate value.
    pub fn from_env() -> Self {
        let secret_auth = get_env("PROVER_SECRET_AUTH");
        let network = get_env("ETH_NETWORK");

        // Checks if an untrusted key is being used for production.
        if &secret_auth == "sample" && &network != "localhost" {
            log::error!("Prover secret for JWT authorization set to 'sample', this is an incorrect value for production");
        }

        Self {
            prepare_data_interval: Duration::from_millis(parse_env("PROVER_PREPARE_DATA_INTERVAL")),
            heartbeat_interval: Duration::from_millis(parse_env("PROVER_HEARTBEAT_INTERVAL")),
            cycle_wait: Duration::from_millis(parse_env("PROVER_CYCLE_WAIT")),
            gone_timeout: Duration::from_millis(parse_env("PROVER_GONE_TIMEOUT")),
            prover_server_address: addr_from_port(parse_env("PROVER_SERVER_PORT")),
            witness_generators: parse_env("WITNESS_GENERATORS"),
            idle_provers: parse_env("IDLE_PROVERS"),
            secret_auth,
        }
    }
}

/// Configuration options for `admin server`.
#[derive(Debug, Clone)]
pub struct AdminServerOptions {
    pub admin_http_server_url: Url,
    pub admin_http_server_address: SocketAddr,
    pub secret_auth: String,
}

impl AdminServerOptions {
    /// Parses the configuration options values from the environment variables.
    /// Panics if any of options is missing or has inappropriate value.
    pub fn from_env() -> Self {
        let secret_auth = get_env("ADMIN_SERVER_SECRET_AUTH");
        let network = get_env("ETH_NETWORK");

        // Checks if an untrusted key is being used for production.
        if &secret_auth == "sample" && &network != "localhost" {
            log::error!("Admin server secret for JWT authorization set to 'sample', this is an incorrect value for production");
        }

        Self {
            admin_http_server_url: parse_env("ADMIN_SERVER_API_URL"),
            admin_http_server_address: addr_from_port(parse_env("ADMIN_SERVER_API_PORT")),
            secret_auth,
        }
    }
}

#[derive(Clone, Debug)]
pub enum TokenPriceSource {
    CoinMarketCap { base_url: Url },
    CoinGecko { base_url: Url },
}

impl TokenPriceSource {
    fn from_env() -> Self {
        match get_env("TOKEN_PRICE_SOURCE").to_lowercase().as_str() {
            "coinmarketcap" => Self::CoinMarketCap {
                base_url: parse_env("COINMARKETCAP_BASE_URL"),
            },
            "coingecko" => Self::CoinGecko {
                base_url: parse_env("COINGECKO_BASE_URL"),
            },
            source => panic!("Unknown token price source: {}", source),
        }
    }
}

/// Configuration options related to generating blocks by state keeper.
/// Each block is generated after a certain amount of miniblock iterations.
/// Miniblock iteration is a routine of processing transactions received so far.
#[derive(Debug, Clone)]
pub struct MiniblockTimings {
    /// Miniblock iteration interval.
    pub miniblock_iteration_interval: Duration,
    /// Max number of miniblocks (produced every period of `TX_MINIBATCH_CREATE_TIME`) if one block.
    pub max_miniblock_iterations: usize,
    /// Max number of miniblocks for block with fast withdraw operations (defaults to `max_minblock_iterations`).
    pub fast_miniblock_iterations: usize,
}

impl MiniblockTimings {
    pub fn from_env() -> Self {
        let fast_miniblock_iterations = if env::var("FAST_BLOCK_MINIBLOCKS_ITERATIONS").is_ok() {
            parse_env("FAST_BLOCK_MINIBLOCKS_ITERATIONS")
        } else {
            parse_env("MINIBLOCKS_ITERATIONS")
        };

        Self {
            miniblock_iteration_interval: Duration::from_millis(parse_env::<u64>(
                "MINIBLOCK_ITERATION_INTERVAL",
            )),
            max_miniblock_iterations: parse_env("MINIBLOCKS_ITERATIONS"),
            fast_miniblock_iterations,
        }
    }
}

/// Configuration options related to fee ticker.
#[derive(Debug)]
pub struct FeeTickerOptions {
    /// Source to fetch token prices from (e.g. CoinGecko or coinmarketcap).
    pub token_price_source: TokenPriceSource,
    /// Fee increase coefficient for fast processing of withdrawal.
    pub fast_processing_coeff: f64,
    /// List of the tokens that aren't acceptable for paying fee in.
    pub disabled_tokens: HashSet<Address>,
    /// Tokens for which subsidies are disabled.
    pub not_subsidized_tokens: HashSet<Address>,
}

impl FeeTickerOptions {
    fn comma_separated_addresses(name: &str) -> HashSet<Address> {
        get_env(name)
            .split(',')
            .map(|p| p.parse().unwrap())
            .collect()
    }

    pub fn from_env() -> Self {
        Self {
            token_price_source: TokenPriceSource::from_env(),
            fast_processing_coeff: parse_env("TICKER_FAST_PROCESSING_COEFF"),
            disabled_tokens: Self::comma_separated_addresses("TICKER_DISABLED_TOKENS"),
            not_subsidized_tokens: Self::comma_separated_addresses("NOT_SUBSIDIZED_TOKENS"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiServerOptions {
    pub rest_api_server_address: SocketAddr,
    pub json_rpc_http_server_address: SocketAddr,
    pub json_rpc_ws_server_address: SocketAddr,
    pub core_server_address: SocketAddr,
    pub core_server_url: String,
    pub api_requests_caches_size: usize,
    /// Fee increase coefficient for fast processing of withdrawal.
    pub forced_exit_minimum_account_age: Duration,
    pub enforce_pubkey_change_fee: bool,
}

impl ApiServerOptions {
    pub fn from_env() -> Self {
        let forced_exit_minimum_account_age =
            Duration::from_secs(parse_env::<u64>("FORCED_EXIT_MINIMUM_ACCOUNT_AGE_SECS"));
        let network = get_env("ETH_NETWORK");

        if forced_exit_minimum_account_age.as_secs() == 0 && &network != "localhost" {
            log::error!("Forced exit minimum account age is set to 0, this is an incorrect value for production");
        }

        Self {
            rest_api_server_address: addr_from_port(parse_env("REST_API_PORT")),
            json_rpc_http_server_address: addr_from_port(parse_env("HTTP_RPC_API_PORT")),
            json_rpc_ws_server_address: addr_from_port(parse_env("WS_API_PORT")),
            core_server_address: addr_from_port(parse_env("PRIVATE_CORE_SERVER_PORT")),
            core_server_url: parse_env("PRIVATE_CORE_SERVER_URL"),
            api_requests_caches_size: parse_env("API_REQUESTS_CACHES_SIZE"),
            forced_exit_minimum_account_age,
            enforce_pubkey_change_fee: parse_env_if_exists("ENFORCE_PUBKEY_CHANGE_FEE")
                .unwrap_or(true),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigurationOptions {
    pub web3_url: String,
    pub genesis_tx_hash: H256,
    pub contract_eth_addr: Address,
    pub governance_eth_addr: Address,
    pub operator_fee_eth_addr: Address,
    pub confirmations_for_eth_event: u64,
    pub available_block_chunk_sizes: Vec<usize>,
    pub max_number_of_withdrawals_per_block: usize,
    pub eth_watch_poll_interval: Duration,
    pub eth_network: String,
    pub miniblock_timings: MiniblockTimings,
    pub prometheus_export_port: u16,
}

impl ConfigurationOptions {
    /// Parses the configuration options values from the environment variables.
    /// Panics if any of options is missing or has inappropriate value.
    pub fn from_env() -> Self {
        let runtime_value = env::var("BLOCK_CHUNK_SIZES").expect("BLOCK_CHUNK_SIZES missing");
        let mut available_block_chunk_sizes = runtime_value
            .split(',')
            .map(|s| usize::from_str(s).unwrap())
            .collect::<Vec<_>>();

        available_block_chunk_sizes.sort_unstable();

        Self {
            web3_url: get_env("WEB3_URL"),
            genesis_tx_hash: parse_env_with("GENESIS_TX_HASH", |s| &s[2..]),
            contract_eth_addr: parse_env_with("CONTRACT_ADDR", |s| &s[2..]),
            governance_eth_addr: parse_env_with("GOVERNANCE_ADDR", |s| &s[2..]),
            operator_fee_eth_addr: parse_env_with("OPERATOR_FEE_ETH_ADDRESS", |s| &s[2..]),
            confirmations_for_eth_event: parse_env("CONFIRMATIONS_FOR_ETH_EVENT"),
            available_block_chunk_sizes,
            max_number_of_withdrawals_per_block: parse_env("MAX_NUMBER_OF_WITHDRAWALS_PER_BLOCK"),
            eth_watch_poll_interval: Duration::from_millis(parse_env::<u64>(
                "ETH_WATCH_POLL_INTERVAL",
            )),
            eth_network: parse_env("ETH_NETWORK"),
            miniblock_timings: MiniblockTimings::from_env(),
            prometheus_export_port: parse_env("PROMETHEUS_EXPORT_PORT"),
        }
    }
}

/// Possible block chunks sizes and corresponding setup powers of two,
/// this is only parameters needed to create verifying contract.
#[derive(Debug)]
pub struct AvailableBlockSizesConfig {
    pub blocks_chunks: Vec<usize>,
    pub blocks_setup_power2: Vec<u32>,
}

impl AvailableBlockSizesConfig {
    pub fn from_env() -> Self {
        let result = Self {
            blocks_chunks: get_env("SUPPORTED_BLOCK_CHUNKS_SIZES")
                .split(',')
                .map(|p| p.parse().unwrap())
                .collect(),
            blocks_setup_power2: get_env("SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS")
                .split(',')
                .map(|p| p.parse().unwrap())
                .collect(),
        };
        assert_eq!(
            result.blocks_chunks.len(),
            result.blocks_setup_power2.len(),
            "block sized and setup powers should have same length, check config file"
        );
        result
    }
}
