//! Load test meant to run against running node.
//! Runs scenario of deposits, withdraws and transfers. Scenario details are
//! specified as input json file. Transactions send concurrently. Program exits
//! successfully if all transactions get verified within configured timeout.

// Built-in import
use std::{
    env,
    ops::Mul,
    sync::Arc,
    thread,
    time::{Duration, Instant},
};
// External uses
use bigdecimal::BigDecimal;
use rand::Rng;
use tokio::runtime::{Builder, Handle};
use web3::{transports::Http, types::U256};
// Workspace uses
use models::{config_options::ConfigurationOptions, node::tx::TxHash};
// Local uses
use self::{
    requests::{account_state_info, ethop_info, send_tx, tx_info},
    test_accounts::TestAccount,
    test_spec::TestSpec,
    tps_counter::{run_tps_counter_printer, TPSCounter},
};

mod requests;
mod test_accounts;
mod test_spec;
mod tps_counter;

const DEPOSIT_TIMEOUT_SEC: u64 = 5 * 60;
// const TPS_MEASURE_WINDOW: usize = 1000;

fn main() {
    env_logger::init();
    let mut tokio_runtime = Builder::new()
        .threaded_scheduler()
        .enable_all()
        .build()
        .expect("failed to construct tokio runtime");

    // Load the configuration options.
    let config = ConfigurationOptions::from_env();

    // Load the test spec.
    let filepath = env::args().nth(1).expect("test spec file not given");
    let test_spec = TestSpec::load(filepath);

    // Create test accounts.
    let (_el, transport) = Http::new(&config.web3_url).expect("http transport start");
    let test_accounts =
        TestAccount::construct_test_accounts(&test_spec.input_accounts, transport, &config);

    // Obtain the Ethereum node JSON RPC address.
    let rpc_addr = env::var("HTTP_RPC_API_ADDR").expect("HTTP_RPC_API_ADDR is missing");
    log::info!("sending transactions");

    // Spawn the TPS counter.
    let tps_counter = Arc::new(TPSCounter::default());
    tokio_runtime.spawn(run_tps_counter_printer(tps_counter.clone()));

    // Send the transactions and block until all of them are sent.
    let sent_txs = tokio_runtime.block_on(send_transactions(
        test_accounts,
        &test_spec,
        &rpc_addr,
        tps_counter,
        tokio_runtime.handle().clone(),
    ));

    // Wait until all the transactions are verified.
    log::info!("waiting for all transactions to be verified");
    tokio_runtime.block_on(wait_for_verify(
        sent_txs,
        Duration::from_secs(test_spec.verify_timeout_sec),
        &rpc_addr,
    ));
    log::info!("loadtest completed.");
}

#[derive(Debug)]
struct SentTransactions {
    op_serial_ids: Vec<u64>,
    tx_hashes: Vec<TxHash>,
}

impl SentTransactions {
    fn merge(&mut self, other: SentTransactions) {
        self.op_serial_ids.extend(other.op_serial_ids.into_iter());
        self.tx_hashes.extend(other.tx_hashes.into_iter());
    }

    fn new() -> SentTransactions {
        SentTransactions {
            op_serial_ids: Vec::new(),
            tx_hashes: Vec::new(),
        }
    }

    fn add_op_id(&mut self, v: u64) {
        self.op_serial_ids.push(v);
    }

    fn add_tx_hash(&mut self, v: TxHash) {
        self.tx_hashes.push(v);
    }
}

// Sends the configured deposits, withdraws and transfers from each account concurrently.
async fn send_transactions(
    test_accounts: Vec<TestAccount>,
    ctx: &TestSpec,
    rpc_addr: &str,
    tps_counter: Arc<TPSCounter>,
    rt_handle: Handle,
) -> SentTransactions {
    let req_client = reqwest::Client::new();

    // Send transactions from every account.
    let join_handles = test_accounts
        .into_iter()
        .map(|account| {
            rt_handle.spawn(send_transactions_from_acc(
                account,
                ctx.clone(),
                rpc_addr.to_string(),
                Arc::clone(&tps_counter),
                req_client.clone(),
            ))
        })
        .collect::<Vec<_>>();

    // Collect all the sent transactions (so we'll be able to wait for their confirmation).
    let mut merged_txs = SentTransactions::new();
    for j in join_handles {
        let sent_txs_result = j.await.expect("Join handle panicked");

        match sent_txs_result {
            Ok(sent_txs) => merged_txs.merge(sent_txs),
            Err(err) => log::warn!("Failed to send txs: {}", err),
        }
    }

    merged_txs
}

// Sends the configured deposits, withdraws and transfer from a single account concurrently.
async fn send_transactions_from_acc(
    test_acc: TestAccount,
    ctx: TestSpec,
    rpc_addr: String,
    tps_counter: Arc<TPSCounter>,
    _req_client: reqwest::Client,
) -> Result<SentTransactions, failure::Error> {
    let mut sent_txs = SentTransactions::new();
    let addr_hex = hex::encode(test_acc.eth_acc.address);
    let wei_in_gwei = BigDecimal::from(1_000_000_000);

    // First of all, we have to update both the Ethereum and ZKSync accounts nonce values.
    test_acc.update_eth_nonce().await?;

    let zknonce = account_state_info(test_acc.zk_acc.address, &rpc_addr)
        .await
        .expect("rpc error")
        .committed
        .nonce;
    test_acc.zk_acc.set_nonce(zknonce);

    // Perform the deposit operation.
    let deposit_amount = BigDecimal::from(ctx.deposit_initial_gwei).mul(&wei_in_gwei);
    let op_id = deposit_single(&test_acc, deposit_amount.clone(), &rpc_addr).await?;

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
        let op_id = deposit_single(&test_acc, amount.mul(&wei_in_gwei), &rpc_addr).await?;
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

    let req_client = reqwest::Client::new();
    for (tx, eth_sign) in tx_queue {
        let tx_hash = send_tx(tx, eth_sign, &rpc_addr, &req_client).await?;
        tps_counter.increment();
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
    rpc_addr: &str,
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
    wait_for_deposit_executed(priority_op.serial_id, rpc_addr).await
}

/// Waits until the deposit priority operation is executed.
async fn wait_for_deposit_executed(serial_id: u64, rpc_addr: &str) -> Result<u64, failure::Error> {
    let mut executed = false;
    // We poll the operation status twice a second until timeout is reached.
    let start = Instant::now();
    let timeout = Duration::from_secs(DEPOSIT_TIMEOUT_SEC);
    let polling_interval = Duration::from_millis(500);

    // Polling cycle.
    while !executed && start.elapsed() < timeout {
        thread::sleep(polling_interval);
        executed = ethop_info(serial_id, rpc_addr).await?.0;
    }

    // Check for the successful execution.
    if !executed {
        failure::bail!("Deposit operation timeout");
    }

    Ok(serial_id)
}

/// Waits for all the priority operations and transactions to become a part of some block and get verified.
async fn wait_for_verify(sent_txs: SentTransactions, timeout: Duration, rpc_addr: &str) {
    let serial_ids = sent_txs.op_serial_ids;

    let start = Instant::now();
    let polling_interval = Duration::from_millis(500);

    // Wait until all the transactions are verified.
    for &id in serial_ids.iter() {
        loop {
            let (executed, verified) = ethop_info(id as u64, rpc_addr)
                .await
                .expect("[wait_for_verify] call ethop_info");
            if executed && verified {
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
            let verified = tx_info(hash.clone(), rpc_addr)
                .await
                .expect("[wait_for_verify] call tx_info");
            if verified {
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
