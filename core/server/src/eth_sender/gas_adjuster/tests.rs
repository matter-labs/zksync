// Local uses
use super::GasAdjuster;
use crate::eth_sender::tests::mock::{default_eth_sender, MockDatabase, MockEthereum};

fn eth_and_db_clients() -> (MockEthereum, MockDatabase) {
    let (eth_sender, _, _) = default_eth_sender();

    (eth_sender.ethereum, eth_sender.db)
}

/// Test for the lower gas limit: it should be a network-suggested price for new transactions,
/// and for stuck transactions it should be the maximum of either price increased by 15% or
/// the network-suggested price.
#[test]
fn lower_gas_limit() {
    let (mut ethereum, db) = eth_and_db_clients();

    let mut gas_adjuster: GasAdjuster<MockEthereum, MockDatabase> = GasAdjuster::new(&db);

    // Set the gas price in Ethereum to 1000.
    ethereum.gas_price = 1000.into();

    // Check that gas price of 1000 is increased to 1150.
    let scaled_gas = gas_adjuster
        .get_gas_price(&ethereum, Some(1000.into()))
        .unwrap();
    assert_eq!(scaled_gas, 1150.into());

    // Check that gas price of 100 is increased to 1000 (price in Ethereum object).
    let scaled_gas = gas_adjuster
        .get_gas_price(&ethereum, Some(100.into()))
        .unwrap();
    assert_eq!(scaled_gas, 1000.into());
}
