// External uses
use zksync_basic_types::U256;
// Local uses
use crate::eth_sender::{
    database::DatabaseAccess,
    gas_adjuster::{parameters::limit_scale_factor, GasAdjuster, GasStatistics},
    tests::mock::{default_eth_sender, MockDatabase, MockEthereum},
};

/// Creates `Ethereum` and `Database` instances for the `GasAdjuster` tests.
fn eth_and_db_clients() -> (MockEthereum, MockDatabase) {
    let (eth_sender, _, _) = default_eth_sender();

    (eth_sender.ethereum, eth_sender.db)
}

/// Scales the gas limit according to the scale factor for GasAdjuster.
fn scale_gas_limit(value: u64) -> u64 {
    let scale = (limit_scale_factor() * 100.0).round() as u64;

    value * scale / 100
}

/// Checks that by default (with no previous tx info provided), GasAdjuster
/// provides the gas price suggested by the client.
#[test]
fn initial_price() {
    let (mut ethereum, db) = eth_and_db_clients();
    let mut gas_adjuster: GasAdjuster<MockEthereum, MockDatabase> = GasAdjuster::new(&db);

    // Vector of ethereum client prices.
    let test_vector = vec![0, 13, db.load_gas_price_limit().unwrap().low_u64()];

    for eth_client_price in test_vector {
        ethereum.gas_price = eth_client_price.into();

        let scaled_gas = gas_adjuster.get_gas_price(&ethereum, None).unwrap();
        assert_eq!(scaled_gas, eth_client_price.into());
    }
}

/// Test for the lower gas limit: it should be a network-suggested price for new transactions,
/// and for stuck transactions it should be the maximum of either price increased by 15% or
/// the network-suggested price.
#[test]
fn lower_gas_limit() {
    let (mut ethereum, db) = eth_and_db_clients();

    let mut gas_adjuster: GasAdjuster<MockEthereum, MockDatabase> = GasAdjuster::new(&db);

    // Test vector of pairs (ethereum client price, price of the last tx, expected price).
    let test_vector = vec![
        (1, 100, 115),   // Client price is too low, increase by 15%
        (200, 100, 200), // Client price is higher, use it
        (115, 100, 115), // Client price == (price + 15%)
        (100, 130, 149), // Fractional result is rounded down
        (0, 0, 0),       // 0 price does not lead to crash
    ];

    for (eth_client_price, previous_price, expected_price) in test_vector {
        // Set the gas price in Ethereum.
        ethereum.gas_price = eth_client_price.into();

        // Check that gas price of 1000 is increased to 1150.
        let scaled_gas = gas_adjuster
            .get_gas_price(&ethereum, Some(previous_price.into()))
            .unwrap();
        assert_eq!(scaled_gas, expected_price.into());
    }
}

// Checks that after re-creation the price limit is restored from the database.
#[test]
fn gas_price_limit_restore() {
    // Price limit to set (should be obtained from the DB by GasAdjuster).
    const PRICE_LIMIT: u64 = 1000;

    let (_, db) = eth_and_db_clients();
    db.update_gas_price_limit(PRICE_LIMIT.into()).unwrap();
    let gas_adjuster: GasAdjuster<MockEthereum, MockDatabase> = GasAdjuster::new(&db);

    assert_eq!(gas_adjuster.get_current_max_price(), PRICE_LIMIT.into());
}

/// Checks that price is clamped according to the current limit.
/// This check works with the initial value only, and does not update it
/// with the gathered stats.
#[test]
fn initial_upper_gas_limit() {
    // Initial price limit to set.
    const PRICE_LIMIT: u64 = 1000;

    let (mut ethereum, db) = eth_and_db_clients();

    db.update_gas_price_limit(PRICE_LIMIT.into()).unwrap();

    let mut gas_adjuster: GasAdjuster<MockEthereum, MockDatabase> = GasAdjuster::new(&db);

    // Set the gas price in Ethereum, which is greater than the current limit.
    ethereum.gas_price = U256::from(PRICE_LIMIT) + 1;

    // Check that gas price of `PRICE_LIMIT` + 1 is clamped to `PRICE_LIMIT`.
    let scaled_gas = gas_adjuster.get_gas_price(&ethereum, None).unwrap();
    assert_eq!(scaled_gas, PRICE_LIMIT.into());

    // Check that gas price is clamped even if both the ethereum client price
    // and last used price are greater than price limit.
    ethereum.gas_price = U256::from(PRICE_LIMIT) * 2;
    let previous_price = U256::from(PRICE_LIMIT) * 2;

    let scaled_gas = gas_adjuster
        .get_gas_price(&ethereum, Some(previous_price))
        .unwrap();
    assert_eq!(scaled_gas, PRICE_LIMIT.into());
}

/// Checks the gas price limit scaling algorithm:
/// We are successively keep requesting the gas price with the
/// ethereum client suggesting the price far beyond the current limit
/// and expect the price limit to be updated according to the schedule.
#[test]
fn gas_price_limit_scaling() {
    // Amount of times we'll call `GasAdjuster::keep_updated`.
    const PRICE_UPDATES: u64 = 5;
    // Amount of samples to gather statistics.
    const N_SAMPLES: usize = GasStatistics::GAS_PRICE_SAMPLES_AMOUNT;
    // Initial price limit to set.
    const PRICE_LIMIT: u64 = 1000;

    let (mut ethereum, db) = eth_and_db_clients();
    db.update_gas_price_limit(PRICE_LIMIT.into()).unwrap();
    let mut gas_adjuster: GasAdjuster<MockEthereum, MockDatabase> = GasAdjuster::new(&db);

    // Set the client price way beyond the limit.
    ethereum.gas_price = U256::from(PRICE_LIMIT * 2);

    let mut expected_price = PRICE_LIMIT;

    // Initial phase: stats are not yet initialized, we are based on the DB limit.
    for _ in 0..N_SAMPLES {
        let suggested_price = gas_adjuster
            .get_gas_price(&ethereum, Some(expected_price.into()))
            .unwrap();

        // Until we call `keep_updated`, the suggested price should not change and should be
        // equal to the limit.
        assert_eq!(suggested_price, expected_price.into());

        // Update the limit.
        gas_adjuster.keep_updated(&ethereum, &db);
    }

    // Stats are gathered. Now they're based on the Ethereum price.
    expected_price = ethereum.gas_price.as_u64();
    for _ in 0..PRICE_UPDATES {
        // Request the gas price N times to gather statistics in GasAdjuster.
        // Each time the limit will be changed, so it's not checked. Instead, we check
        // the expected limit after `N_SAMPLES` below (it's simpler).
        for _ in 0..N_SAMPLES {
            gas_adjuster.keep_updated(&ethereum, &db);
        }

        // Check that new limit is scaled old limit (and also check that it's stored in the DB).
        let new_limit = db.load_gas_price_limit().unwrap();
        assert_eq!(new_limit, scale_gas_limit(expected_price).into());

        // Update the expected price for the next round.
        expected_price = new_limit.low_u64();

        // Also, scale up the price reported by the ethereum.
        ethereum.gas_price = U256::from(expected_price);
    }
}

/// Checks that if the price suggested by the Ethereum client is below the price limit,
/// the limit is calculated as (average of samples) * scale_factor.
#[test]
#[ignore] // TODO: Disabled as currently the limit is calculated based on the network price rather than used txs samples.
fn gas_price_limit_average_basis() {
    // Increases the gas price value by 15%.
    fn increase_gas_price(value: u64) -> u64 {
        value * 115 / 100
    }

    // Amount of times we'll call `GasAdjuster::keep_updated`.
    const PRICE_UPDATES: u64 = 5;
    // Amount of samples to gather statistics.
    const N_SAMPLES: usize = GasStatistics::GAS_PRICE_SAMPLES_AMOUNT;
    // Initial price limit to set.
    const PRICE_LIMIT: u64 = 10000;
    // Price suggested by Ethereum client;
    const SUGGESTED_PRICE: u64 = 10;

    let (mut ethereum, db) = eth_and_db_clients();
    db.update_gas_price_limit(PRICE_LIMIT.into()).unwrap();
    let mut gas_adjuster: GasAdjuster<MockEthereum, MockDatabase> = GasAdjuster::new(&db);

    // Set the client price way beyond the limit.
    ethereum.gas_price = SUGGESTED_PRICE.into();

    let mut expected_price = SUGGESTED_PRICE;
    let mut current_limit = PRICE_LIMIT;

    for _ in 0..PRICE_UPDATES {
        let mut samples_sum = 0;

        // Request the gas price N times to gather statistics in GasAdjuster.
        for _ in 0..N_SAMPLES {
            let suggested_price = gas_adjuster
                .get_gas_price(&ethereum, Some(expected_price.into()))
                .unwrap();

            let increased_price = increase_gas_price(expected_price);

            expected_price = if increased_price <= current_limit {
                // Increased price is lower than limit, it should be used.
                increased_price
            } else {
                // Price limit exceeded, clamp to the limit.
                current_limit
            };
            assert_eq!(suggested_price, expected_price.into());

            // Update the sum of this samples set.
            samples_sum += expected_price;
        }

        // Keep the limit updated (it should become (avg of prices) * (scale factor).
        gas_adjuster.keep_updated(&ethereum, &db);

        // Check that new limit is based on the average of previous N samples.
        let new_limit = db.load_gas_price_limit().unwrap();

        current_limit = scale_gas_limit(samples_sum / N_SAMPLES as u64);
        assert_eq!(new_limit, current_limit.into());
    }
}

/// Checks that if the gas price limit is never achieved, it never increased as well.
#[test]
fn gas_price_limit_preservation() {
    // Amount of times we'll call `GasAdjuster::keep_updated`.
    // The value is lower than in tests above, since the limit must not change.
    const PRICE_UPDATES: u64 = 2;
    // Amount of samples to gather statistics.
    const N_SAMPLES: usize = GasStatistics::GAS_PRICE_SAMPLES_AMOUNT;
    // Price suggested by Ethereum client;
    const SUGGESTED_PRICE: u64 = 10;
    // Price limit to set: it's based on the suggested price, so it won't ever change.
    let price_limit = scale_gas_limit(SUGGESTED_PRICE);

    let (mut ethereum, db) = eth_and_db_clients();
    db.update_gas_price_limit(price_limit.into()).unwrap();
    let mut gas_adjuster: GasAdjuster<MockEthereum, MockDatabase> = GasAdjuster::new(&db);

    // Set the client price way beyond the limit.
    ethereum.gas_price = SUGGESTED_PRICE.into();

    for _ in 0..PRICE_UPDATES {
        // Request the gas price N times to gather statistics in GasAdjuster.
        for _ in 0..N_SAMPLES {
            // Every time we get the new price (without old price provided), so no scaling
            // involved, every time an Ethereum client price is provided (since it's lower
            // than the limit).
            let suggested_price = gas_adjuster.get_gas_price(&ethereum, None).unwrap();
            assert_eq!(suggested_price, SUGGESTED_PRICE.into());
        }

        // Keep the limit updated (it should not change).
        gas_adjuster.keep_updated(&ethereum, &db);
        let new_limit = db.load_gas_price_limit().unwrap();
        assert_eq!(new_limit, price_limit.into());
    }
}
