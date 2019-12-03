//! Transaction sending policy.
//!
//! Goal is to handle stuck transactions.
//! When we try to commit operation to ETH we select nonce, gas price, sign transaction and
//! watch for its confirmations.
//!
//! If transaction is not confirmed for a while we increase gas price and do the same, but we
//! keep list of all sent transactions for one particular operations, since we can't be sure which
//! one will be commited so we track all of them.
//!
//! Note: make sure to save signed tx to db before sending it to ETH, this way we can be sure
//! that state is always recoverable.

// Built-in uses
use std::collections::VecDeque;
use std::str::FromStr;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};
// External uses
use bigdecimal::BigDecimal;
use ff::{PrimeField, PrimeFieldRepr};
use futures::{sync::mpsc as fmpsc, Future};
use web3::contract::Options;
use web3::transports::Http;
use web3::types::{Transaction, TransactionId, TransactionReceipt, H256, U256};
use web3::Transport;
// Workspace uses
use crate::{ConfigurationOptions, ThreadPanicNotify};
use eth_client::{ETHClient, SignedCallResult};
use models::abi::FRANKLIN_CONTRACT;
use models::{Action, ActionType, Operation};
use storage::ConnectionPool;

const EXPECTED_WAIT_TIME_BLOCKS: u64 = 30;
// TODO: fix rare nonce bug, when pending nonce is not equal to real pending nonce.
const MAX_UNCONFIRMED_TX: usize = 1;
const TX_POLL_PERIOD: Duration = Duration::from_secs(5);
const WAIT_CONFIRMATIONS: u64 = 10;

#[derive(Debug, Clone)]
enum SignedTxState {
    Unsent,
    Pending { deadline_block: u64 },
    Executed { confirmations: u64, status: u64 },
    Confirmed { status: u64 },
}

impl SignedTxState {
    fn confirmed_status(&self) -> Option<u64> {
        match self {
            SignedTxState::Confirmed { status } => Some(*status),
            _ => None,
        }
    }

    fn stuck(&self, current_block: u64) -> bool {
        match self {
            SignedTxState::Pending { deadline_block } => current_block > *deadline_block,
            _ => false,
        }
    }
}

#[derive(Debug, Clone)]
enum OperationState {
    Pending,
    InProgress {
        current_transaction: (SignedCallResult, SignedTxState),
        previous_transactions: Vec<H256>,
    },
    Commited {
        hash: H256,
        success: bool,
    },
}

impl OperationState {
    fn is_unsent(&self) -> bool {
        match self {
            OperationState::Pending { .. } => true,
            _ => false,
        }
    }
}

struct ETHSender<T: Transport> {
    unconfirmed_ops: VecDeque<(Operation, OperationState)>,
    db_pool: ConnectionPool,
    eth_client: ETHClient<T>,
}

impl<T: Transport> ETHSender<T> {
    fn new(db_pool: ConnectionPool, eth_client: ETHClient<T>) -> Self {
        let mut sender = Self {
            eth_client,
            unconfirmed_ops: VecDeque::new(),
            db_pool,
        };
        sender.restore_state().expect("Eth sender state restore");
        sender
    }

    fn restore_state(&mut self) -> Result<(), failure::Error> {
        let storage = self.db_pool.access_storage()?;
        self.unsent_ops
            .extend(storage.load_unsent_ops()?.into_iter());
        let stored_sent_ops = storage.load_sent_unconfirmed_ops()?;
        self.unconfirmed_ops = stored_sent_ops
            .into_iter()
            .map(|(op, eth_ops)| {
                let current_transaction = eth_ops
                    .iter()
                    .max_by_key(|eth_op| eth_op.gas_price.clone())
                    .map(|stored_eth_tx| {
                        let signed_tx = self
                            .sign_operation_tx(
                                &op,
                                Options::with(|opt| {
                                    opt.gas_price = Some(
                                        U256::from_dec_str(&stored_eth_tx.gas_price.to_string())
                                            .expect("U256 parse error"),
                                    );
                                    opt.nonce = Some(U256::from(stored_eth_tx.nonce as u32));
                                }),
                            )
                            .expect("Failed when restoring tx");
                        let tx_state = SignedTxState::Pending {
                            deadline_block: stored_eth_tx.deadline_block as u64,
                        };
                        (signed_tx, tx_state)
                    })
                    .unwrap();
                let previous_transactions = eth_ops
                    .into_iter()
                    .filter_map(|eth_op| {
                        let eth_op_hash = eth_op.tx_hash[2..].parse().unwrap();
                        if eth_op_hash != current_transaction.0.hash {
                            Some(eth_op_hash)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<H256>>();
                (
                    op,
                    OperationState::InProgress {
                        current_transaction,
                        previous_transactions,
                    },
                )
            })
            .collect();
        Ok(())
    }

    fn run(&mut self, rx_for_eth: Receiver<Operation>, mut op_notify: fmpsc::Sender<Operation>) {
        loop {
            let last_poll_end_time = Instant::now();

            while let Ok(op) = rx_for_eth.try_recv() {
                self.unsent_ops.push_back(op);
            }

            while self.unconfirmed_ops.len() < MAX_UNCONFIRMED_TX && !self.unsent_ops.is_empty() {
                if let Some(op) = self.unsent_ops.pop_front() {
                    self.unconfirmed_ops
                        .push_back((op, OperationState::Pending));
                }
            }

            // Try sending all txs in queue.
            let mut sending_error = false;
            self.unconfirmed_ops = std::mem::replace(&mut self.unconfirmed_ops, VecDeque::new())
                .into_iter()
                .map(|(op, mut op_state)| {
                    if op_state.is_unsent() && !sending_error {
                        if let Err(e) = self.drive_to_completion(&op, &mut op_state) {
                            sending_error = true;
                            warn!("Error while sending unsent op: {}", e);
                        }
                    }
                    (op, op_state)
                })
                .collect();

            while let Some((op, mut op_state)) = self.unconfirmed_ops.pop_front() {
                self.drive_to_completion(&op, &mut op_state)
                    .map_err(|e| {
                        warn!("Error while trying to complete uncommited op: {}", e);
                    })
                    .unwrap_or_default();

                if let OperationState::Commited { hash, success } = &op_state {
                    info!(
                        "Operation {}, {}  block: {}, commited, tx: {:#x} success: {}",
                        op.id.unwrap(),
                        op.action.to_string(),
                        op.block.block_number,
                        hash,
                        success
                    );
                    if !success {
                        panic!("Operation failed");
                    }
                    if op.action.get_type() == ActionType::VERIFY {
                        // we notify about verify only when commit is confirmed on the ethereum
                        op_notify
                            .try_send(op)
                            .map_err(|e| warn!("Failed notify about verify op confirmation: {}", e))
                            .unwrap_or_default();
                    }
                } else {
                    self.unconfirmed_ops.push_front((op, op_state));
                    break;
                }
            }

            let since_last_poll_time = last_poll_end_time.elapsed();
            if since_last_poll_time < TX_POLL_PERIOD {
                std::thread::sleep(TX_POLL_PERIOD - since_last_poll_time);
            }
        }
    }

    fn commited_nonce(&self) -> Result<u64, failure::Error> {
        Ok(self.eth_client.current_nonce().wait().map(|n| n.as_u64())?)
    }
    fn block_number(&self) -> Result<u64, failure::Error> {
        Ok(self
            .eth_client
            .web3
            .eth()
            .block_number()
            .wait()
            .map(|n| n.as_u64())?)
    }

    fn get_tx_status(&self, hash: &H256) -> Result<Option<(u64, u64)>, failure::Error> {
        let block_number = self.block_number()?;
        match self
            .eth_client
            .web3
            .eth()
            .transaction_receipt(*hash)
            .wait()?
        {
            Some(TransactionReceipt {
                block_number: Some(tx_block_number),
                status: Some(status),
                ..
            }) => {
                let confirmations = block_number.saturating_sub(tx_block_number.as_u64());
                Ok(Some((confirmations, status.as_u64())))
            }
            _ => Ok(None),
        }
    }

    fn save_signed_tx_to_db(
        &self,
        op: &Operation,
        signed_tx: &SignedCallResult,
        deadline_block: u64,
    ) -> Result<(), failure::Error> {
        let storage = self.db_pool.access_storage()?;
        let gas_price = BigDecimal::from_str(&signed_tx.gas_price.to_string()).unwrap();
        Ok(storage.save_operation_eth_tx(
            op.id.unwrap(),
            signed_tx.hash,
            deadline_block,
            signed_tx.nonce.as_u32(),
            gas_price,
        )?)
    }

    fn save_completed_tx_to_db(&self, hash: &H256) -> Result<(), failure::Error> {
        let storage = self.db_pool.access_storage()?;
        Ok(storage.confirm_eth_tx(hash)?)
    }

    fn drive_tx_to_completion(
        &self,
        op: &Operation,
        tx: &SignedCallResult,
        tx_state: &mut SignedTxState,
    ) -> Result<(), failure::Error> {
        match tx_state {
            SignedTxState::Unsent => {
                let deadline_block = self.block_number()? + EXPECTED_WAIT_TIME_BLOCKS;
                self.save_signed_tx_to_db(op, tx, deadline_block)?;
                *tx_state = SignedTxState::Pending { deadline_block };
                let hash = self.eth_client.send_raw_tx(tx.raw_tx.clone())?;
                assert_eq!(hash, tx.hash, "bug in signed tx offline hash calculation");
            }
            SignedTxState::Pending { .. } => {
                let tx_info = self
                    .eth_client
                    .web3
                    .eth()
                    .transaction(TransactionId::Hash(tx.hash))
                    .wait()?;
                match tx_info {
                    Some(Transaction {
                        block_number: Some(_),
                        ..
                    }) => {
                        // transaction was confirmed, get receipt
                        if let Some((confirmations, status)) = self.get_tx_status(&tx.hash)? {
                            *tx_state = SignedTxState::Executed {
                                confirmations,
                                status,
                            };
                        }
                    }
                    Some(_) => {}
                    None => {
                        // transaction is unknown? try resending
                        self.eth_client
                            .send_raw_tx(tx.raw_tx.clone())
                            .unwrap_or_default();
                    }
                }
            }
            SignedTxState::Executed {
                confirmations,
                status,
            } => {
                if let Some((new_confirmations, new_status)) = self.get_tx_status(&tx.hash)? {
                    *confirmations = new_confirmations;
                    *status = new_status;
                }
                // here reorg can be handled. (receipt of tx was lost => reorg)

                if *confirmations >= WAIT_CONFIRMATIONS {
                    *tx_state = SignedTxState::Confirmed { status: *status }
                }
            }
            SignedTxState::Confirmed { .. } => {}
        }
        Ok(())
    }

    fn drive_to_completion(
        &self,
        op: &Operation,
        state: &mut OperationState,
    ) -> Result<(), failure::Error> {
        match state {
            OperationState::Pending => {
                let signed_tx = self.sign_op_tx(op)?;
                *state = OperationState::InProgress {
                    current_transaction: (signed_tx, SignedTxState::Unsent),
                    previous_transactions: Vec::new(),
                };
                return self.drive_to_completion(op, state);
            }
            OperationState::InProgress {
                current_transaction: (tx, tx_state),
                previous_transactions,
            } => {
                let current_nonce = self.commited_nonce()?;

                self.drive_tx_to_completion(op, tx, tx_state)?;

                let current_block = self.block_number()?;
                if tx_state.stuck(current_block) {
                    let new_tx = self.resign_op_tx(op, tx)?;
                    previous_transactions.push(tx.hash);
                    *tx = new_tx;
                    *tx_state = SignedTxState::Unsent;
                    self.drive_tx_to_completion(op, tx, tx_state)?;
                }

                if let Some(status) = tx_state.confirmed_status() {
                    let success = status == 1;
                    if success {
                        self.save_completed_tx_to_db(&tx.hash)?;
                    }
                    *state = OperationState::Commited {
                        success,
                        hash: tx.hash,
                    };
                } else if current_nonce > tx.nonce.as_u64() {
                    // some older tx was commited.
                    let new_state = previous_transactions.iter().find_map(|old_tx| {
                        if let Ok(Some((confirmations, status))) = self.get_tx_status(old_tx) {
                            if confirmations >= WAIT_CONFIRMATIONS {
                                let success = status == 1;
                                if success {
                                    self.save_completed_tx_to_db(&old_tx).ok()?;
                                }
                                return Some(OperationState::Commited {
                                    success,
                                    hash: *old_tx,
                                });
                            }
                        }
                        None
                    });
                    if let Some(new_state) = new_state {
                        *state = new_state;
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn sign_op_tx(&self, op: &Operation) -> Result<SignedCallResult, failure::Error> {
        self.sign_operation_tx(op, Options::default())
    }

    fn resign_op_tx(
        &self,
        op: &Operation,
        old_tx: &SignedCallResult,
    ) -> Result<SignedCallResult, failure::Error> {
        let new_gas_price = {
            let network_price = self.eth_client.get_gas_price()?;
            // replacement price should be at least 10% higher, we make it 15% higher.
            let replacement_price = (old_tx.gas_price * U256::from(15)) / U256::from(100);
            std::cmp::max(network_price, replacement_price)
        };

        let tx_options = Options::with(move |opt| {
            opt.gas_price = Some(new_gas_price);
            opt.nonce = Some(old_tx.nonce);
        });

        self.sign_operation_tx(op, tx_options)
    }

    fn sign_operation_tx(
        &self,
        op: &Operation,
        tx_options: Options,
    ) -> Result<SignedCallResult, failure::Error> {
        match &op.action {
            Action::Commit => {
                let mut be_bytes = [0u8; 32];
                op.block
                    .new_root_hash
                    .into_repr()
                    .write_be(be_bytes.as_mut())
                    .expect("Write commit bytes");
                let root = H256::from(be_bytes);
                let public_data = op.block.get_eth_public_data();
                debug!(
                    "public_data for block_number {}: {}",
                    op.block.block_number,
                    hex::encode(&public_data)
                );
                // function commitBlock(uint32 _blockNumber, uint24 _feeAccount, bytes32 _newRoot, bytes calldata _publicData)
                self.eth_client.sign_call_tx(
                    "commitBlock",
                    (
                        u64::from(op.block.block_number),
                        u64::from(op.block.fee_account),
                        root,
                        public_data,
                    ),
                    tx_options,
                )
            }
            Action::Verify { proof } => {
                // function verifyBlock(uint32 _blockNumber, uint256[8] calldata proof) external {
                self.eth_client.sign_call_tx(
                    "verifyBlock",
                    (u64::from(op.block.block_number), *proof.clone()),
                    tx_options,
                )
            }
        }
    }
}

pub fn start_eth_sender(
    pool: ConnectionPool,
    panic_notify: Sender<bool>,
    op_notify_sender: fmpsc::Sender<Operation>,
    config_options: ConfigurationOptions,
) -> Sender<Operation> {
    let (tx_for_eth, rx_for_eth) = channel::<Operation>();

    std::thread::Builder::new()
        .name("eth_sender".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);
            let (_event_loop, transport) =
                Http::new(&config_options.web3_url).expect("failed to start web3 transport");

            let abi_string = serde_json::Value::from_str(FRANKLIN_CONTRACT)
                .unwrap()
                .get("abi")
                .unwrap()
                .to_string();
            let eth_client = ETHClient::new(
                transport,
                abi_string,
                config_options.operator_eth_addr,
                config_options.operator_private_key,
                config_options.contract_eth_addr,
                config_options.chain_id,
                config_options.gas_price_factor,
            );

            let mut eth_sender = ETHSender::new(pool, eth_client);
            eth_sender.run(rx_for_eth, op_notify_sender);
        })
        .expect("Eth sender thread");

    tx_for_eth
}
