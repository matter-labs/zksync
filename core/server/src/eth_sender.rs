//! Transaction processing policy.
//! only one pending tx at a time.
//! poll for tx execu
//!

use bigdecimal::BigDecimal;
use eth_client::{CallResult, ETHClient, SignedCallResult};
use failure::format_err;
use ff::{PrimeField, PrimeFieldRepr};
use futures::Future;
use models::abi::TEST_PLASMA2_ALWAYS_VERIFY;
use models::{Action, Operation};
use num_traits::cast::FromPrimitive;
use std::collections::VecDeque;
use std::str::FromStr;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};
use storage::ConnectionPool;
use web3::contract::Options;
use web3::transports::Http;
use web3::types::{TransactionReceipt, H256};
use web3::Transport;

const EXPECTED_WAIT_TIME_BLOCKS: u64 = 10;
const MAX_UNCONFIRMED_TX: usize = 5;
const TX_POLL_PERIOD: Duration = Duration::from_secs(5);

enum TxState {
    Unsent(SignedCallResult),
    Pending {
        hash: H256,
        signed_tx: SignedCallResult,
        deadline_block: u64,
    },
    /// Success or execution code
    Executed(Result<(), i64>),
}

struct ETHSender<T: Transport> {
    eth_client: ETHClient<T>,
    unsent_ops: VecDeque<Operation>,
    unconfirmed_ops: VecDeque<(Operation, Vec<TxState>)>,
    db_pool: ConnectionPool,
}

impl<T: Transport> ETHSender<T> {
    fn new(transport: T, db_pool: ConnectionPool) -> Self {
        let eth_client = {
            let abi_string = serde_json::Value::from_str(TEST_PLASMA2_ALWAYS_VERIFY)
                .unwrap()
                .get("abi")
                .unwrap()
                .to_string();

            ETHClient::new(transport, abi_string)
        };
        let mut sender = Self {
            eth_client,
            unsent_ops: VecDeque::new(),
            unconfirmed_ops: VecDeque::new(),
            db_pool,
        };
        sender.restore_state().expect("Eth sender state restore");
        sender
    }

    fn restore_state(&mut self) -> Result<(), failure::Error> {
        let current_nonce = self
            .eth_client
            .current_nonce()
            .wait()
            .map_err(|_| format_err!("get nonce error"))?;

        let storage = self.db_pool.access_storage()?;
        info!(
            "Starting eth_sender: sender = {:#x}, current_nonce = {}",
            self.eth_client.sender_account, current_nonce
        );

        self.unsent_ops.extend(storage.load_unsent_ops()?);

        self.unconfirmed_ops = storage
            .load_sent_unconfirmed_ops()?
            .into_iter()
            .map(|(op, eth_op)| UnconfirmedOperation {
                hash: eth_op.tx_hash[2..].parse().unwrap(),
                deadline_block: eth_op.deadline_block as u64,
                op,
            })
            .collect();

        Ok(())
    }

    fn run(&mut self, rx_for_eth: Receiver<Operation>) {
        let storage = self.db_pool.access_storage().expect("db access");

        let mut last_tx_poll_time = Instant::now();

        loop {
            while let Ok(op) = rx_for_eth.try_recv() {
                self.unsent_ops.push_back(op);
            }

            let block_number = match self.eth_client.web3.eth().block_number().wait() {
                Ok(block) => block.as_u64(),
                Err(e) => {
                    warn!("Failed to get block number: {}", e);
                    continue;
                }
            };

            while self.unconfirmed_ops.len() < MAX_UNCONFIRMED_TX && !self.unsent_ops.is_empty() {
                if let Some(op) = self.unsent_ops.pop_front() {
                    let signed_tx = match self.sign_operation_tx(&op) {
                        Ok(singed_tx) => signed_tx,
                        Err(e) => {
                            error!("Failed to form signed ETH transaction: {}", e);
                            self.unsent_ops.push_front(op);
                            continue;
                        }
                    };

                    // TODO: storage save signed transaction to db.

                    self.unconfirmed_ops
                        .push_back((op, vec![TxState::Unsent(tx_call_result)]));
                }
            }

            // check pending txs
            let since_last_poll = last_tx_poll_time.elapsed();
            if since_last_poll < TX_POLL_PERIOD {
                std::thread::sleep(TX_POLL_PERIOD - since_last_poll);
            }
            while let Some(UnconfirmedOperation {
                hash,
                deadline_block,
                op,
            }) = self.unconfirmed_ops.front()
            {
                match self.eth_client.web3.eth().transaction_receipt(*hash).wait() {
                    Ok(Some(TransactionReceipt {
                        status: Some(status),
                        gas_used,
                        ..
                    })) => {
                        if status.as_u64() != 1 {
                            panic!("Operation {} failed, tx_hash: {:#x}", op.id.unwrap(), hash)
                        }

                        info!(
                            "Operation {} confirmed, tx_hash: {:#x}, gas_used: {:?}",
                            op.id.unwrap(),
                            hash,
                            gas_used
                        );
                        storage.confirm_eth_tx(hash).expect("db fail");
                        self.unconfirmed_ops.pop_front();
                    }
                    // op is not confirmed yet
                    Ok(_) => {
                        if block_number > *deadline_block {
                            info!(
                                "Operation {} is not commited before deadline, resending tx",
                                op.id.unwrap()
                            );
                            unimplemented!();
                        }
                    }
                    Err(e) => {
                        error!("Error while checking pending transaction: {}", e);
                        break;
                    }
                }
            }
            last_tx_poll_time = Instant::now();
        }
    }

    fn sign_operation_tx(&mut self, op: &Operation) -> Result<SignedCallResult, failure::Error> {
        match &op.action {
            Action::Commit => {
                let mut be_bytes = [0u8; 32];
                op.block
                    .new_root_hash
                    .into_repr()
                    .write_be(be_bytes.as_mut())
                    .expect("Write commit bytes");
                let root = H256::from(be_bytes);
                debug!(
                    "public_data for block_number {}: {:x?}",
                    op.block.block_number,
                    op.block.get_eth_public_data()
                );
                // function commitBlock(uint32 _blockNumber, uint24 _feeAccount, bytes32 _newRoot, bytes calldata _publicData)
                self.eth_client.sign_call_tx(
                    "commitBlock",
                    (
                        u64::from(op.block.block_number),
                        u64::from(op.block.fee_account),
                        root,
                        op.block.get_eth_public_data(),
                    ),
                    Options::default(),
                )
            }
            Action::Verify { proof } => {
                // function verifyBlock(uint32 _blockNumber, uint256[8] calldata proof) external {
                self.eth_client.sign_call_tx(
                    "verifyBlock",
                    (u64::from(op.block.block_number), *proof.clone()),
                    Options::default(),
                )
            }
        }
    }
}

pub fn start_eth_sender(pool: ConnectionPool) -> Sender<Operation> {
    let (tx_for_eth, rx_for_eth) = channel::<Operation>();

    std::thread::Builder::new()
        .name("eth_sender".to_string())
        .spawn(move || {
            let web3_url = std::env::var("WEB3_URL").expect("WEB3_URL env var not found");
            let (_event_loop, transport) =
                Http::new(&web3_url).expect("failed to start web3 transport");

            let mut eth_sender = ETHSender::new(transport, pool);
            eth_sender.run(rx_for_eth);
        })
        .expect("Eth sender thread");

    tx_for_eth
}
