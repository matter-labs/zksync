// Built-in deps
use std::{collections::VecDeque, marker::PhantomData, time::Instant};
// External deps
use zksync_basic_types::U256;
// Local deps
use crate::{database::Database, ethereum_interface::EthereumInterface};

mod parameters;

// TODO: Tests are commented for now as DB traits aren't implemented yet.
// #[cfg(test)]
// mod tests;

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
pub(super) struct GasAdjuster<ETH: EthereumInterface> {
    /// Collected statistics about recently used gas prices.
    statistics: GasStatistics,
    /// Timestamp of the last maximum gas price update.
    last_price_renewal: Instant,
    /// Timestamp of the last sample added to the `statistics`.
    last_sample_added: Instant,

    _etherum_client: PhantomData<ETH>,
}

impl<ETH: EthereumInterface> GasAdjuster<ETH> {
    pub async fn new(db: &Database) -> Self {
        let mut connection = db
            .acquire_connection()
            .await
            .expect("Unable to connect to DB");
        let gas_price_limit = db
            .load_gas_price_limit(&mut connection)
            .await
            .expect("Can't load the gas price limit");
        Self {
            statistics: GasStatistics::new(gas_price_limit),
            last_price_renewal: Instant::now(),
            last_sample_added: Instant::now(),

            _etherum_client: PhantomData,
        }
    }

    /// Calculates a new gas amount for the replacement of the stuck tx.
    /// Replacement price is usually suggested to be at least 10% higher, we make it 15% higher.
    pub async fn get_gas_price(
        &mut self,
        ethereum: &ETH,
        old_tx_gas_price: Option<U256>,
    ) -> Result<U256, anyhow::Error> {
        let network_price = ethereum.gas_price().await?;

        let scaled_price = if let Some(old_price) = old_tx_gas_price {
            // Stuck transaction, scale it up.
            self.scale_up(old_price, network_price)
        } else {
            // New transaction, use the network price as the base.
            network_price
        };

        // Now, cut the price if it's too big.
        let price = self.limit_max(scaled_price);

        if price == self.get_current_max_price() {
            // We're suggesting the max price, so we must notify the log
            // entry about it.
            log::warn!("Maximum possible gas price will be used: <{}>", price);
        }

        // FIXME: Currently instead of using sent txs as samples, we use the gas prices suggested by
        // the Ethereum node.
        // // Report used price to be gathered by the statistics module.
        // self.statistics.add_sample(price);

        Ok(price)
    }

    /// Performs an actualization routine for `GasAdjuster`:
    /// This method is intended to be invoked periodically, and it updates the
    /// current max gas price limit according to the configurable update interval.
    pub async fn keep_updated(&mut self, ethereum: &ETH, db: &Database) {
        if self.last_sample_added.elapsed() >= parameters::sample_adding_interval() {
            // Report the current price to be gathered by the statistics module.
            match ethereum.gas_price().await {
                Ok(network_price) => {
                    self.statistics.add_sample(network_price);

                    self.last_sample_added = Instant::now();
                }
                Err(err) => {
                    log::warn!("Cannot add the sample gas price: {}", err);
                }
            }
        }

        if self.last_price_renewal.elapsed() >= parameters::limit_update_interval() {
            // It's time to update the maximum price.
            let scale_factor = parameters::limit_scale_factor();
            self.statistics.update_limit(scale_factor);
            self.last_price_renewal = Instant::now();

            // Update the value in the database as well.
            let mut connection = match db.acquire_connection().await {
                Ok(connection) => connection,
                Err(err) => {
                    log::warn!("Cannot update the gas limit value in the database: {}", err);
                    return;
                }
            };
            let result = db
                .update_gas_price_params(
                    &mut connection,
                    self.statistics.get_limit(),
                    self.get_average_gas_price(),
                )
                .await;

            if let Err(err) = result {
                // Inability of update the value in the DB is not critical as it's not
                // an essential logic part, so just report the error to the log.
                log::warn!("Cannot update the gas limit value in the database: {}", err);
            }
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

    /// Returns current max gas price that can be used to send transactions.
    pub fn get_current_max_price(&self) -> U256 {
        self.statistics.get_limit()
    }

    /// Get estimate of the average gas prices used for past transactions based on the current gas_limit.
    pub fn get_average_gas_price(&self) -> U256 {
        let scale_factor = parameters::limit_scale_factor();
        let divider = U256::from((scale_factor * 100.0f64).round() as u64);
        let multiplier = U256::from(100);
        self.statistics.get_limit() * multiplier / divider
    }
}

/// Helper structure responsible for collecting the data about recent transactions,
/// calculating the average gas price, and providing the gas price limit.
#[derive(Debug)]
struct GasStatistics {
    samples: VecDeque<U256>,
    current_sum: U256,
    current_max_price: U256,
}

impl GasStatistics {
    /// Amount of entries in the gas price statistics pool.
    const GAS_PRICE_SAMPLES_AMOUNT: usize = 10;

    pub fn new(initial_max_price: U256) -> Self {
        Self {
            samples: VecDeque::with_capacity(Self::GAS_PRICE_SAMPLES_AMOUNT),
            current_sum: 0.into(),
            current_max_price: initial_max_price,
        }
    }

    pub fn add_sample(&mut self, price: U256) {
        if self.samples.len() >= Self::GAS_PRICE_SAMPLES_AMOUNT {
            let popped_price = self.samples.pop_front().unwrap();

            self.current_sum -= popped_price;
        }

        self.samples.push_back(price);
        self.current_sum += price;
    }

    pub fn update_limit(&mut self, scale_factor: f64) {
        if self.samples.len() < Self::GAS_PRICE_SAMPLES_AMOUNT {
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

    pub fn get_limit(&self) -> U256 {
        self.current_max_price
    }
}
