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
use serde::Serialize;
use tokio::runtime::{Builder, Handle};
use web3::{transports::Http, types::U256};
// Workspace uses
use models::{
    config_options::ConfigurationOptions,
    node::tx::{FranklinTx, PackedEthSignature, TxHash},
};
// Local uses
use self::{
    requests::{account_state_info, ethop_info, tx_info},
    test_accounts::TestAccount,
    test_spec::{AccountInfo, TestSpec},
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

    let config = ConfigurationOptions::from_env();
    let filepath = env::args().nth(1).expect("test spec file not given");
    let test_spec = TestSpec::load(filepath);
    let (_el, transport) = Http::new(&config.web3_url).expect("http transport start");
    let test_accounts =
        TestAccount::construct_test_accounts(&test_spec.input_accounts, transport, &config);
    let rpc_addr = env::var("HTTP_RPC_API_ADDR").expect("HTTP_RPC_API_ADDR is missing");
    log::info!("sending transactions");

    let tps_counter = Arc::new(TPSCounter::default());
    tokio_runtime.spawn(run_tps_counter_printer(tps_counter.clone()));

    let sent_txs = tokio_runtime.block_on(send_transactions(
        test_accounts,
        &test_spec,
        &rpc_addr,
        tps_counter,
        tokio_runtime.handle().clone(),
    ));
    log::info!("waiting for all transactions to be verified");
    tokio_runtime.block_on(wait_for_verify(
        sent_txs,
        Duration::from_secs(test_spec.verify_timeout_sec),
        &rpc_addr,
    ));
    log::info!("loadtest completed.");
}

struct SentTransactions {
    op_serial_ids: Vec<u32>,
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

    fn add_op_id(&mut self, v: u32) {
        self.op_serial_ids.push(v);
    }

    fn add_tx_hash(&mut self, v: TxHash) {
        self.tx_hashes.push(v);
    }
}

// sends confugured deposits, withdraws and transfers from each account concurrently.
async fn send_transactions(
    test_accounts: Vec<TestAccount>,
    ctx: &TestSpec,
    rpc_addr: &str,
    tps_counter: Arc<TPSCounter>,
    rt_handle: Handle,
) -> SentTransactions {
    let req_client = reqwest::Client::new();

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

// sends configured deposits, withdraws and transfer from a single account concurrently.
async fn send_transactions_from_acc(
    test_acc: TestAccount,
    ctx: TestSpec,
    rpc_addr: String,
    tps_counter: Arc<TPSCounter>,
    _req_client: reqwest::Client,
) -> Result<SentTransactions, failure::Error> {
    let mut sent_txs = SentTransactions::new();

    let addr_hex = hex::encode(test_acc.eth_acc.address);
    update_eth_nonce(&test_acc).await?;
    let zknonce = account_state_info(test_acc.zk_acc.address, &rpc_addr)
        .await
        .expect("rpc error")
        .committed
        .nonce;
    test_acc.zk_acc.set_nonce(zknonce);

    let wei_in_gwei = BigDecimal::from(1_000_000_000);
    let op_id = deposit_single(
        &test_acc,
        BigDecimal::from(ctx.deposit_initial_gwei).mul(&wei_in_gwei),
        &rpc_addr,
    )
    .await?;
    log::info!("account {} made initial deposit", addr_hex);
    sent_txs.add_op_id(op_id);
    let mut tx_queue = Vec::with_capacity((ctx.n_transfers + ctx.n_withdraws) as usize);
    tx_queue.push((sign_change_pubkey(&test_acc), None));
    log::info!("sending {} transactions", addr_hex);
    for _ in 0..ctx.n_deposits {
        let amount = rand_amount(ctx.deposit_from_amount_gwei, ctx.deposit_to_amount_gwei);
        let op_id = deposit_single(&test_acc, amount.mul(&wei_in_gwei), &rpc_addr).await?;
        sent_txs.add_op_id(op_id);
    }

    log::info!("Signing transactions");
    for _ in 0..ctx.n_transfers {
        let amount = rand_amount(ctx.transfer_from_amount_gwei, ctx.transfer_to_amount_gwei);
        let signed_transfer =
            sign_transfer_single(&test_acc, &ctx.input_accounts, amount.mul(&wei_in_gwei));
        tx_queue.push(signed_transfer);
    }
    for _ in 0..ctx.n_withdraws {
        let amount = rand_amount(ctx.withdraw_from_amount_gwei, ctx.withdraw_to_amount_gwei);
        let signed_withdraw = sign_withdraw_single(&test_acc, amount.mul(&wei_in_gwei));
        tx_queue.push(signed_withdraw)
    }
    log::info!("Done signing transactions");

    let req_client = reqwest::Client::new();
    for (tx, eth_sign) in tx_queue {
        let tx_hash = send_tx(tx, eth_sign, &rpc_addr, &req_client).await?;
        tps_counter.increment();
        sent_txs.add_tx_hash(tx_hash);
    }

    log::info!("Sending txs");

    Ok(sent_txs)
}

// generates random amount for transaction within given range [from, to).
fn rand_amount(from: u64, to: u64) -> BigDecimal {
    let amount = rand::thread_rng().gen_range(from, to);
    BigDecimal::from(amount)
}

// updates current ethereum nonces from eth node.
async fn update_eth_nonce(test_acc: &TestAccount) -> Result<(), failure::Error> {
    let mut nonce = test_acc.eth_nonce.lock().await;
    let v = test_acc
        .eth_acc
        .main_contract_eth_client
        .pending_nonce()
        .await
        .map_err(|e| failure::format_err!("update_eth_nonce: {}", e))?;
    *nonce = v.as_u32();
    Ok(())
}

fn sign_change_pubkey(ta: &TestAccount) -> FranklinTx {
    FranklinTx::ChangePubKey(Box::new(
        ta.zk_acc.create_change_pubkey_tx(None, true, false),
    ))
}

// deposits to contract and waits for node to execute it.
async fn deposit_single(
    test_acc: &TestAccount,
    deposit_amount: BigDecimal,
    rpc_addr: &str,
) -> Result<u32, failure::Error> {
    let nonce = {
        let mut n = test_acc.eth_nonce.lock().await;
        *n += 1;
        Some(U256::from(*n - 1))
    };
    let po = test_acc
        .eth_acc
        .deposit_eth(deposit_amount, &test_acc.zk_acc.address, nonce)
        .await?;
    wait_for_deposit_executed(po.serial_id, rpc_addr).await
}

async fn wait_for_deposit_executed(serial_id: u64, rpc_addr: &str) -> Result<u32, failure::Error> {
    let mut executed = false;
    // 5 min wait
    let start = Instant::now();
    let timeout = Duration::from_secs(DEPOSIT_TIMEOUT_SEC);
    let check_period = Duration::from_secs(1);
    while start.elapsed() < timeout {
        let (ex, _) = ethop_info(serial_id, rpc_addr).await?;
        if ex {
            executed = true;
            break;
        }
        thread::sleep(check_period);
    }
    if executed {
        return Ok(serial_id as u32);
    }
    failure::bail!("timeout")
}

// sends withdraw.
fn sign_withdraw_single(
    test_acc: &TestAccount,
    amount: BigDecimal,
) -> (FranklinTx, Option<PackedEthSignature>) {
    let (tx, eth_signature) = test_acc.zk_acc.sign_withdraw(
        0, // ETH
        "ETH",
        amount,
        BigDecimal::from(0),
        &test_acc.eth_acc.address,
        None,
        true,
    );
    (FranklinTx::Withdraw(Box::new(tx)), Some(eth_signature))
}

// sends transfer tx to a random receiver.
fn sign_transfer_single(
    from: &TestAccount,
    test_accounts: &[AccountInfo],
    amount: BigDecimal,
) -> (FranklinTx, Option<PackedEthSignature>) {
    let to = {
        let mut to_idx = rand::thread_rng().gen_range(0, test_accounts.len() - 1);
        while test_accounts[to_idx].address == from.zk_acc.address {
            to_idx = rand::thread_rng().gen_range(0, test_accounts.len() - 1);
        }
        test_accounts[to_idx].address
    };
    let (tx, eth_signature) = from.zk_acc.sign_transfer(
        0, // ETH
        "ETH",
        amount,
        BigDecimal::from(0),
        &to,
        None,
        true,
    );
    (FranklinTx::Transfer(Box::new(tx)), Some(eth_signature))
}

#[derive(Serialize)]
struct SubmitTxMsg {
    id: String,
    method: String,
    jsonrpc: String,
    params: Vec<serde_json::Value>,
}

impl SubmitTxMsg {
    fn new(tx: FranklinTx, eth_signature: Option<PackedEthSignature>) -> Self {
        let mut params = Vec::new();
        params.push(serde_json::to_value(tx).expect("serialization fail"));
        if let Some(eth_signature) = eth_signature {
            params.push(serde_json::to_value(eth_signature).expect("serialization fail"));
        }
        Self {
            id: "1".to_owned(),
            method: "tx_submit".to_owned(),
            jsonrpc: "2.0".to_owned(),
            params,
        }
    }
}

// sends tx to server json rpc endpoint.
async fn send_tx(
    tx: FranklinTx,
    eth_signature: Option<PackedEthSignature>,
    rpc_addr: &str,
    client: &reqwest::Client,
) -> Result<TxHash, failure::Error> {
    let tx_hash = tx.hash();
    // let nonce = tx.nonce();
    let msg = SubmitTxMsg::new(tx, eth_signature);

    // let instant = Instant::now();
    let res = client.post(rpc_addr).json(&msg).send().await?;
    if res.status() != reqwest::StatusCode::OK {
        failure::bail!("non-ok response: {}", res.status());
    }
    //    log::trace!("tx: {}", res.text().await.unwrap());
    Ok(tx_hash)
}

// waits for all priority operations and transactions to become part of some block and get verified.
async fn wait_for_verify(sent_txs: SentTransactions, timeout: Duration, rpc_addr: &str) {
    let start = Instant::now();
    let serial_ids = sent_txs.op_serial_ids;
    let sleep_period = Duration::from_millis(500);
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
                panic!("[wait_for_verify] timeout")
            }
            thread::sleep(sleep_period);
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
                panic!("[wait_for_verify] timeout")
            }
            thread::sleep(sleep_period);
        }
    }
}
