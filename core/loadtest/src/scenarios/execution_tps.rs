//! Load test meant to run against running node.
//! Runs scenario of deposits, withdraws and transfers. Scenario details are
//! specified as input json file. Transactions are sent concurrently. Program exits
//! successfully if all transactions get verified within configured timeout.
//!
//! This scenario measures the execution TPS.

// Built-in import
use std::{
    ops::Mul,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
// External uses
use bigdecimal::BigDecimal;
use futures::future::join_all;
use rand::Rng;
use tokio::runtime::Handle;
use web3::types::U256;
// Workspace uses
use models::node::tx::TxHash;
// Local uses
use crate::{
    rpc_client::RpcClient,
    scenarios::ScenarioContext,
    sent_transactions::SentTransactions,
    test_accounts::TestAccount,
    test_spec::TestSpec,
    tps_counter::{run_tps_counter_printer, TPSCounter},
};

const DEPOSIT_TIMEOUT_SEC: u64 = 5 * 60;
const TX_EXECUTION_TIMEOUT_SEC: u64 = 5 * 60;

/// Runs the execution TPS scenario:
/// sends the different types of transactions, and measures the TPS for the txs execution
/// (not including the verification).
pub fn run_scenario(mut ctx: ScenarioContext) {
    let verify_timeout_sec = Duration::from_secs(ctx.ctx.verify_timeout_sec).clone();
    let rpc_addr = ctx.rpc_addr.clone();

    let rpc_client = RpcClient::new(&rpc_addr);

    // Obtain the Ethereum node JSON RPC address.
    log::info!("Starting the loadtest");

    // Spawn the TPS counter.
    ctx.rt
        .spawn(run_tps_counter_printer(ctx.tps_counter.clone()));

    // Send the transactions and block until all of them are sent.
    let sent_txs = ctx.rt.block_on(send_transactions(
        ctx.test_accounts,
        rpc_client.clone(),
        ctx.ctx,
        ctx.rt.handle().clone(),
        ctx.tps_counter,
    ));

    // Wait until all the transactions are verified.
    log::info!("Waiting for all transactions to be verified");
    ctx.rt
        .block_on(wait_for_verify(sent_txs, verify_timeout_sec, &rpc_client));
    log::info!("Loadtest completed.");
}

// Sends the configured deposits, withdraws and transfers from each account concurrently.
async fn send_transactions(
    test_accounts: Vec<TestAccount>,
    rpc_client: RpcClient,
    ctx: TestSpec,
    rt_handle: Handle,
    tps_counter: Arc<TPSCounter>,
) -> SentTransactions {
    // Send transactions from every account.

    let join_handles: Vec<_> = test_accounts
        .into_iter()
        .map(|account| {
            rt_handle.spawn(send_transactions_from_acc(
                account,
                ctx.clone(),
                rpc_client.clone(),
            ))
        })
        .collect();

    // Collect all the sent transactions (so we'll be able to wait for their confirmation).
    let mut merged_txs = SentTransactions::new();

    let mut txs_await_handles = Vec::new();

    // Await for the transaction send routines, and create the transaction execution routines
    // (which will measure the execution TPS).
    for j in join_handles {
        let sent_txs_result = j.await.expect("Join handle panicked");

        match sent_txs_result {
            Ok(sent_txs) => {
                let task_handle = rt_handle.spawn(await_txs_execution(
                    rt_handle.clone(),
                    sent_txs.tx_hashes.clone(),
                    Arc::clone(&tps_counter),
                    rpc_client.clone(),
                ));

                txs_await_handles.push(task_handle);

                merged_txs.merge(sent_txs);
            }
            Err(err) => log::warn!("Failed to send txs: {}", err),
        }
    }

    // Await transaction execution routines.
    for j in txs_await_handles {
        j.await.expect("Join handle panicked");
    }

    merged_txs
}

// Sends the configured deposits, withdraws and transfer from a single account concurrently.
async fn send_transactions_from_acc(
    test_acc: TestAccount,
    ctx: TestSpec,
    rpc_client: RpcClient,
) -> Result<SentTransactions, failure::Error> {
    let mut sent_txs = SentTransactions::new();
    let addr_hex = hex::encode(test_acc.eth_acc.address);
    let wei_in_gwei = BigDecimal::from(1_000_000_000);

    // First of all, we have to update both the Ethereum and ZKSync accounts nonce values.
    test_acc.update_eth_nonce().await?;

    let zknonce = rpc_client
        .account_state_info(test_acc.zk_acc.address)
        .await
        .expect("rpc error")
        .committed
        .nonce;
    test_acc.zk_acc.set_nonce(zknonce);

    // Perform the deposit operation.
    let deposit_amount = BigDecimal::from(ctx.deposit_initial_gwei).mul(&wei_in_gwei);
    let op_id = deposit_single(&test_acc, deposit_amount.clone(), &rpc_client).await?;

    log::info!(
        "Account {}: initial deposit completed (amount: {})",
        addr_hex,
        deposit_amount
    );
    sent_txs.add_op_id(op_id);

    log::info!(
        "Account {}: performing {} deposit operations",
        addr_hex,
        ctx.n_deposits,
    );

    // Add the deposit operations.
    for _ in 0..ctx.n_deposits {
        let amount = rand_amount(ctx.deposit_from_amount_gwei, ctx.deposit_to_amount_gwei);
        let op_id = deposit_single(&test_acc, amount.mul(&wei_in_gwei), &rpc_client).await?;
        sent_txs.add_op_id(op_id);
    }

    // Create a queue for all the transactions to send.
    // First, we will create and sign all the transactions, and then we will send all the
    // prepared transactions.
    let n_change_pubkeys = 1;
    let txs_amount = (n_change_pubkeys + ctx.n_transfers + ctx.n_withdraws) as usize;
    let mut tx_queue = Vec::with_capacity(txs_amount);

    log::info!(
        "Account {}: preparing {} transactions to send",
        addr_hex,
        txs_amount,
    );

    // Add the `ChangePubKey` operation.
    tx_queue.push((test_acc.sign_change_pubkey(), None));

    // Add the transfer operations.
    for _ in 0..ctx.n_transfers {
        let amount = rand_amount(ctx.transfer_from_amount_gwei, ctx.transfer_to_amount_gwei);
        let signed_transfer =
            test_acc.sign_transfer_to_random(&ctx.input_accounts, amount.mul(&wei_in_gwei));
        tx_queue.push(signed_transfer);
    }
    // Add the withdraw operations.
    for _ in 0..ctx.n_withdraws {
        let amount = rand_amount(ctx.withdraw_from_amount_gwei, ctx.withdraw_to_amount_gwei);
        let signed_withdraw = test_acc.sign_withdraw_single(amount.mul(&wei_in_gwei));
        tx_queue.push(signed_withdraw)
    }

    log::info!(
        "Account {}: preparing transactions completed, sending...",
        addr_hex
    );

    for (tx, eth_sign) in tx_queue {
        let tx_hash = rpc_client.send_tx(tx, eth_sign).await?;
        sent_txs.add_tx_hash(tx_hash);
    }

    log::info!("Account: {}: all the transactions are sent", addr_hex);

    Ok(sent_txs)
}

// generates random amount for transaction within given range [from, to).
fn rand_amount(from: u64, to: u64) -> BigDecimal {
    let amount = rand::thread_rng().gen_range(from, to);
    BigDecimal::from(amount)
}

/// Deposits to contract and waits for node to execute it.
async fn deposit_single(
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
async fn wait_for_deposit_executed(
    serial_id: u64,
    rpc_client: &RpcClient,
) -> Result<u64, failure::Error> {
    let mut executed = false;
    // We poll the operation status twice a second until timeout is reached.
    let start = Instant::now();
    let timeout = Duration::from_secs(DEPOSIT_TIMEOUT_SEC);
    let polling_interval = Duration::from_millis(500);

    // Polling cycle.
    while !executed && start.elapsed() < timeout {
        thread::sleep(polling_interval);
        let state = rpc_client.ethop_info(serial_id).await?;
        executed = state.executed;
    }

    // Check for the successful execution.
    if !executed {
        failure::bail!("Deposit operation timeout");
    }

    Ok(serial_id)
}

async fn await_txs_execution(
    rt_handle: Handle,
    tx_hashes: Vec<TxHash>,
    tps_counter: Arc<TPSCounter>,
    rpc_client: RpcClient,
) {
    async fn await_tx(tx_hash: TxHash, rpc_client: RpcClient, tps_counter: Arc<TPSCounter>) {
        let timeout = Duration::from_secs(TX_EXECUTION_TIMEOUT_SEC);
        // Small polling interval, so we won't wait too long between confirmation
        // check attempts.
        let polling_interval = Duration::from_millis(100);
        let start = Instant::now();
        loop {
            let state = rpc_client
                .tx_info(tx_hash.clone())
                .await
                .expect("[wait_for_verify] call tx_info");

            if state.executed {
                tps_counter.increment();
                break;
            }
            if start.elapsed() > timeout {
                panic!("[wait_for_verify] Timeout")
            }
            thread::sleep(polling_interval);
        }
    }

    let task_handles: Vec<_> = tx_hashes
        .into_iter()
        .map(|hash| rt_handle.spawn(await_tx(hash, rpc_client.clone(), tps_counter.clone())))
        .collect();

    join_all(task_handles).await;
}

/// Waits for all the priority operations and transactions to become a part of some block and get verified.
async fn wait_for_verify(sent_txs: SentTransactions, timeout: Duration, rpc_client: &RpcClient) {
    let serial_ids = sent_txs.op_serial_ids;

    let start = Instant::now();
    let polling_interval = Duration::from_millis(500);

    // Wait until all the transactions are verified.
    for &id in serial_ids.iter() {
        loop {
            let state = rpc_client
                .ethop_info(id as u64)
                .await
                .expect("[wait_for_verify] call ethop_info");
            if state.executed && state.verified {
                log::debug!("deposit (serial_id={}) is verified", id);
                break;
            }
            if start.elapsed() > timeout {
                panic!("[wait_for_verify] Timeout")
            }
            thread::sleep(polling_interval);
        }
    }

    let tx_hashes = sent_txs.tx_hashes;
    for hash in tx_hashes.iter() {
        loop {
            let state = rpc_client
                .tx_info(hash.clone())
                .await
                .expect("[wait_for_verify] call tx_info");
            if state.verified {
                log::debug!("{} is verified", hash.to_string());
                break;
            }
            if start.elapsed() > timeout {
                panic!("[wait_for_verify] Timeout")
            }
            thread::sleep(polling_interval);
        }
    }
}
