//! Common functions shared by different scenarios.

// Built-in uses
use std::{
    iter::Iterator,
    time::{Duration, Instant},
};
// External uses
use num::BigUint;
use rand::Rng;
use tokio::time;
// Workspace uses
use zksync::{ethereum::PriorityOpHolder, utils::biguint_to_u256, Provider};
// Local uses
use crate::{monitor::Monitor, test_accounts::TestWallet};

const DEPOSIT_TIMEOUT_SEC: u64 = 5 * 60;

// generates random amount for transaction within given range [from, to).
pub fn rand_amount(from: u64, to: u64) -> BigUint {
    let amount = rand::thread_rng().gen_range(from, to);
    BigUint::from(amount)
}

/// Deposits to contract and waits for node to execute it.
pub async fn deposit_single(
    test_wallet: &TestWallet,
    deposit_amount: BigUint,
) -> Result<u64, anyhow::Error> {
    let deposit_amount = biguint_to_u256(deposit_amount);

    let tx_hash = test_wallet
        .eth_provider
        .deposit(
            TestWallet::TOKEN_NAME,
            deposit_amount,
            test_wallet.address(),
        )
        .await?;

    let receipt = test_wallet.eth_provider.wait_for_tx(tx_hash).await?;
    let priority_op = receipt
        .priority_op()
        .expect("no priority op log in deposit");

    wait_for_deposit_executed(priority_op.serial_id, &test_wallet.monitor).await
}

/// Waits until the deposit priority operation is executed.
pub async fn wait_for_deposit_executed(
    serial_id: u64,
    monitor: &Monitor,
) -> Result<u64, anyhow::Error> {
    let mut executed = false;
    // We poll the operation status twice a second until timeout is reached.
    let start = Instant::now();
    let timeout = Duration::from_secs(DEPOSIT_TIMEOUT_SEC);
    let polling_interval = Duration::from_millis(100);
    let mut timer = time::interval(polling_interval);

    // Polling cycle.
    while !executed && start.elapsed() < timeout {
        timer.tick().await;
        let state = monitor.provider.ethop_info(serial_id as u32).await?;
        executed = state.executed;
    }

    // Check for the successful execution.
    if !executed {
        anyhow::bail!("Deposit operation timeout");
    }

    Ok(serial_id)
}

/// Waits for all the priority operations and transactions to become a part of some block and get verified.
pub async fn wait_for_verify(
    sent_txs: SentTransactions,
    timeout: Duration,
    provider: &Provider,
) -> Result<(), anyhow::Error> {
    let serial_ids = sent_txs.op_serial_ids;

    let start = Instant::now();
    let polling_interval = Duration::from_millis(250);
    let mut timer = time::interval(polling_interval);

    // Wait until all the transactions are verified.
    for &id in serial_ids.iter() {
        loop {
            let state = provider.ethop_info(id as u32).await?;
            if state.is_verified() {
                log::debug!("deposit (serial_id={}) is verified", id);
                break;
            }
            if start.elapsed() > timeout {
                anyhow::bail!("[wait_for_verify] Timeout")
            }
            timer.tick().await;
        }
    }

    let tx_hashes = sent_txs.tx_hashes;
    for hash in tx_hashes.iter() {
        loop {
            let state = provider.tx_info(*hash).await?;
            if state.is_verified() {
                log::debug!("{} is verified", hash.to_string());
                break;
            }
            if start.elapsed() > timeout {
                anyhow::bail!("[wait_for_verify] Timeout")
            }
            timer.tick().await;
        }
    }

    Ok(())
}

/// An iterator similar to `.iter().chunks(..)`, but supporting multiple
/// different chunk sizes. Size of yielded batches is chosen one-by-one
/// from the provided list of sizes (preserving their order).
///
/// For example, if chunk sizes array is `[10, 20]` and the iterator is
/// created over an array of 43 elements, sizes of batches will be 10,
/// 20, 10 again and then 3 (remaining elements).
#[derive(Debug)]
pub struct DynamicChunks<T, I>
where
    I: Iterator<Item = T>,
{
    iterable: I,
    chunk_sizes: Vec<usize>,
    chunk_size_id: usize,
}

impl<T, I> DynamicChunks<T, I>
where
    I: Iterator<Item = T>,
{
    pub fn new<J>(iterable: J, chunk_sizes: &[usize]) -> Self
    where
        J: IntoIterator<Item = T, IntoIter = I>,
    {
        assert!(!chunk_sizes.is_empty());

        Self {
            iterable: iterable.into_iter(),
            chunk_sizes: chunk_sizes.to_vec(),
            chunk_size_id: 0,
        }
    }
}

impl<T, I> Iterator for DynamicChunks<T, I>
where
    I: Iterator<Item = T>,
{
    type Item = Vec<T>;

    fn next(&mut self) -> Option<Vec<T>> {
        let chunk_size = self.chunk_sizes[self.chunk_size_id];
        self.chunk_size_id = (self.chunk_size_id + 1) % self.chunk_sizes.len();

        let mut items = Vec::new();
        for _ in 0..chunk_size {
            if let Some(value) = self.iterable.next() {
                items.push(value);
            } else {
                break;
            }
        }

        if items.is_empty() {
            None
        } else {
            Some(items)
        }
    }
}
