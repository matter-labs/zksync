// Built-in deps
use std::{
    collections::VecDeque,
    marker::PhantomData,
    time::{Duration, Instant},
};
// External deps
use web3::types::U256;
// Workspace deps
use models::config_options::parse_env;
// Local deps
use crate::eth_sender::{database::DatabaseAccess, ethereum_interface::EthereumInterface};

/// Constant to be used as the maximum gas price upon a first launch
/// of the server until the gas price statistics are gathered.
/// Currently set to 200 gwei.
const INITIAL_MAX_GAS_PRICE: u64 = 200 * 10e9 as u64;
/// Amount of entries in the gas price statistics pool.
const GAS_PRICE_SAMPLES_AMOUNT: usize = 10;
/// Name of the environment variable responsible for the `max_gas_price` renewing interval.
const MAX_GAS_PRICE_RENEWAL_INTERVAL_VAR: &'static str = "ETH_MAX_GAS_PRICE_RENEWAL_INTERVAL";
/// Name of the environment variable responsible for the `max_gas_price` scaling multiplier.
const MAX_GAS_PRICE_SCALE_FACTOR_VAR: &'static str = "ETH_MAX_GAS_PRICE_SCALE_FACTOR";

/// Gas adjuster is an entity capable of scaling the gas price for
/// all the Ethereum transactions.
///
/// Gas price is adjusted with an upper limit, which is configured
/// dynamically based on the average gas price observed within past
/// sent transactions, and with a lower limit (for managing "stuck"
/// transactions only), which guarantees that we will increase the
/// gas price for transactions that were not mined by the network
/// within a reasonable time.
#[derive(Debug)]
pub(super) struct GasAdjuster<ETH: EthereumInterface, DB: DatabaseAccess> {
    /// Collected statistics about recently used gas prices.
    statistics: GasStatistics,
    /// Timestamp of the last maximum gas price update.
    last_price_renewal: Instant,

    _etherum_client: PhantomData<ETH>,
    _db: PhantomData<DB>,
}

impl<ETH: EthereumInterface, DB: DatabaseAccess> GasAdjuster<ETH, DB> {
    pub fn new() -> Self {
        Self {
            statistics: GasStatistics::new(INITIAL_MAX_GAS_PRICE.into()),
            last_price_renewal: Instant::now(),

            _etherum_client: PhantomData,
            _db: PhantomData,
        }
    }

    // /// Calculates a new gas amount for the replacement of the stuck tx.
    // /// Replacement price should be at least 10% higher, we make it 15% higher.
    pub fn get_gas_price(
        &mut self,
        ethereum: &ETH,
        old_tx_gas_price: Option<U256>,
    ) -> Result<U256, failure::Error> {
        let network_price = ethereum.gas_price()?;

        let scaled_price = if let Some(old_price) = old_tx_gas_price {
            // Stuck transaction, scale it up.
            self.scale_up(old_price, network_price)
        } else {
            // New transaction, use the network price as the base.
            network_price
        };

        // Now, cut the price if it's too big.
        let price = self.limit_max(scaled_price);

        // Report used price to be gathered by the statistics module.
        self.statistics.add_sample(price);

        Ok(price)
    }

    /// Performs an actualization routine for `GasAdjuster`:
    /// This method is intended to be invoked periodically, and it updates the
    /// current max gas price limit according to the configurable update interval.
    pub fn keep_updated(&mut self) {
        if self.last_price_renewal.elapsed() >= self.get_max_price_interval() {
            // It's time to update the maximum price.
            let scale_factor = self.get_max_price_scale();
            self.statistics.update_max_price(scale_factor);
            self.last_price_renewal = Instant::now();
        }
    }

    fn scale_up(&self, price_to_scale: U256, current_network_price: U256) -> U256 {
        let replacement_price = (price_to_scale * U256::from(115)) / U256::from(100);
        std::cmp::max(current_network_price, replacement_price)
    }

    fn limit_max(&self, price: U256) -> U256 {
        let limit = self.get_current_max_price();

        std::cmp::min(price, limit)
    }

    fn get_current_max_price(&self) -> U256 {
        self.statistics.get_max_price()
    }

    /// Obtains the interval for renewing the maximum gas price.
    ///
    /// This value is not cached internally, as it may be changed for the already running
    /// server by an administrator. This may be required if existing settings aren't flexible
    /// enough to match the current network price.
    fn get_max_price_interval(&self) -> Duration {
        let renew_interval: u64 = parse_env(MAX_GAS_PRICE_RENEWAL_INTERVAL_VAR);

        Duration::from_secs(renew_interval)
    }

    /// Obtains the scaling factor for the maximum gas price.
    ///
    /// This value is not cached internally, as it may be changed for the already running
    /// server by an administrator. This may be required if existing settings aren't flexible
    /// enough to match the current network price.
    fn get_max_price_scale(&self) -> f64 {
        parse_env(MAX_GAS_PRICE_SCALE_FACTOR_VAR)
    }
}

#[derive(Debug)]
struct GasStatistics {
    samples: VecDeque<U256>,
    current_sum: U256,
    current_max_price: U256,
}

impl GasStatistics {
    pub fn new(initial_max_price: U256) -> Self {
        Self {
            samples: VecDeque::with_capacity(GAS_PRICE_SAMPLES_AMOUNT),
            current_sum: 0.into(),
            current_max_price: initial_max_price,
        }
    }

    pub fn add_sample(&mut self, price: U256) {
        if self.samples.len() >= GAS_PRICE_SAMPLES_AMOUNT {
            let popped_price = self.samples.pop_front().unwrap();

            self.current_sum -= popped_price;
        }

        self.samples.push_back(price);
        self.current_sum += price;
    }

    pub fn update_max_price(&mut self, scale_factor: f64) {
        if self.samples.len() < GAS_PRICE_SAMPLES_AMOUNT {
            // Not enough data, do nothing.
            return;
        }

        // Since `U256` cannot be multiplied by `f64`, we replace this operation
        // with two:
        // Instead of `a` * `b`, we do `a` * `U256::from(b * 100)` / `U256::from(100)`.
        //
        // This approach assumes that the scale factor is not too precise, e.g. `1.5` or `5.0`,
        // but not `3.14159265`.
        let multiplier = (scale_factor * 100.0f64).round() as u64;
        let multiplier = U256::from(multiplier);

        let divider = U256::from(100);

        let average_price = self.current_sum / self.samples.len();

        self.current_max_price = average_price * multiplier / divider;
    }

    pub fn get_max_price(&self) -> U256 {
        self.current_max_price
    }
}
