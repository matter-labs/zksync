//! Common functions shared by different scenarios.

// Built-in deps
use std::time::{Duration, Instant};
// External deps
use bigdecimal::BigDecimal;
use rand::Rng;
use tokio::time;
use web3::types::U256;
// Local deps
use crate::{rpc_client::RpcClient, test_accounts::TestAccount};

const DEPOSIT_TIMEOUT_SEC: u64 = 5 * 60;

// generates random amount for transaction within given range [from, to).
pub fn rand_amount(from: u64, to: u64) -> BigDecimal {
    let amount = rand::thread_rng().gen_range(from, to);
    BigDecimal::from(amount)
}

/// Deposits to contract and waits for node to execute it.
pub async fn deposit_single(
    test_acc: &TestAccount,
    deposit_amount: BigDecimal,
    rpc_client: &RpcClient,
) -> Result<u64, failure::Error> {
    let nonce = {
        let mut n = test_acc.eth_nonce.lock().await;
        *n += 1;
        Some(U256::from(*n - 1))
    };
    let priority_op = test_acc
        .eth_acc
        .deposit_eth(deposit_amount, &test_acc.zk_acc.address, nonce)
        .await?;
    wait_for_deposit_executed(priority_op.serial_id, &rpc_client).await
}

/// Waits until the deposit priority operation is executed.
pub async fn wait_for_deposit_executed(
    serial_id: u64,
    rpc_client: &RpcClient,
) -> Result<u64, failure::Error> {
    let mut executed = false;
    // We poll the operation status twice a second until timeout is reached.
    let start = Instant::now();
    let timeout = Duration::from_secs(DEPOSIT_TIMEOUT_SEC);
    let polling_interval = Duration::from_millis(500);
    let mut timer = time::interval(polling_interval);

    // Polling cycle.
    while !executed && start.elapsed() < timeout {
        timer.tick().await;
        let state = rpc_client.ethop_info(serial_id).await?;
        executed = state.executed;
    }

    // Check for the successful execution.
    if !executed {
        failure::bail!("Deposit operation timeout");
    }

    Ok(serial_id)
}
