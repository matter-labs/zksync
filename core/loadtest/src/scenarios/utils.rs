//! Common functions shared by different scenarios.

// Built-in deps
use std::time::{Duration, Instant};
// External deps
use num::BigUint;
use rand::Rng;
use tokio::time;
use web3::types::U256;
// Local deps
use crate::{
    rpc_client::RpcClient, sent_transactions::SentTransactions, test_accounts::TestAccount,
};

const DEPOSIT_TIMEOUT_SEC: u64 = 5 * 60;

// generates random amount for transaction within given range [from, to).
pub fn rand_amount(from: u64, to: u64) -> BigUint {
    let amount = rand::thread_rng().gen_range(from, to);
    BigUint::from(amount)
}

/// Deposits to contract and waits for node to execute it.
pub async fn deposit_single(
    test_acc: &TestAccount,
    deposit_amount: BigUint,
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
    let polling_interval = Duration::from_millis(100);
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

/// Waits for all the priority operations and transactions to become a part of some block and get verified.
pub async fn wait_for_verify(
    sent_txs: SentTransactions,
    timeout: Duration,
    rpc_client: &RpcClient,
) -> Result<(), failure::Error> {
    let serial_ids = sent_txs.op_serial_ids;

    let start = Instant::now();
    let polling_interval = Duration::from_millis(250);
    let mut timer = time::interval(polling_interval);

    // Wait until all the transactions are verified.
    for &id in serial_ids.iter() {
        loop {
            let state = rpc_client.ethop_info(id as u64).await?;
            if state.executed && state.verified {
                log::debug!("deposit (serial_id={}) is verified", id);
                break;
            }
            if start.elapsed() > timeout {
                failure::bail!("[wait_for_verify] Timeout")
            }
            timer.tick().await;
        }
    }

    let tx_hashes = sent_txs.tx_hashes;
    for hash in tx_hashes.iter() {
        loop {
            let state = rpc_client.tx_info(hash.clone()).await?;
            if state.verified {
                log::debug!("{} is verified", hash.to_string());
                break;
            }
            if start.elapsed() > timeout {
                failure::bail!("[wait_for_verify] Timeout")
            }
            timer.tick().await;
        }
    }

    Ok(())
}
