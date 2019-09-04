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
use web3::types::{Transaction, TransactionId, TransactionReceipt, H256, U256};
use web3::Transport;

const EXPECTED_WAIT_TIME_BLOCKS: u64 = 30;
const MAX_UNCONFIRMED_TX: usize = 5;
const TX_POLL_PERIOD: Duration = Duration::from_secs(5);
const WAIT_CONFIRMATIONS: u64 = 10;

enum SignedTxState {
    Unsent,
    Pending { deadline_block: u64 },
    Executed { confirmations: u64, status: i64 },
    Confirmed { status: i64 },
}

impl SingedTxState {
    fn confirmed_status(&self) -> Option<i64> {
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
    fn is_commited(&self) -> bool {
        match self {
            OperationState::Commited => true,
            _ => false,
        }
    }
    fn is_unsent(&self) -> bool {
        match self {
            OperationState::Pending => true,
            _ => false,
        }
    }
}

struct ETHSender<T: Transport> {
    unsent_ops: VecDeque<Operation>,
    unconfirmed_ops: VecDeque<(Operation, OperationState)>,
    db_pool: ConnectionPool,
    eth_client: ETHClient<T>,
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
        let storage = self.db_pool.access_storage()?;
        self.unsent_ops
            .extend(storage.load_unsent_ops()?.into_iter());
        let stored_sent_ops = storage.load_sent_unconfirmed_ops()?;
        self.unconfirmed_ops = stored_sent_ops.into_iter().map(
            |(op, eth_ops)| {
                let current_transaction = eth_ops
                    .iter()
                    .max_by_key(|eth_op| eth_op.gas_price).map(
                    |stored_eth_tx|{
                        let signed_tx = self.sign_operation_tx(&op, Options::with(|opt| {
                            opt.gas_price = Some(U256::from_dec_str(&stored_eth_tx.gas_price.to_string()).expect("U256 parse error"));
                            opt.nonce = Some(U256::from(stored_eth_tx.nonce as u32));
                        })).expect("Failed when restoring tx");
                        let tx_state = SignedTxState::Pending { deadline_block: stored_eth_tx.deadline_block as u64};
                        (signed_tx, tx_state)
                }).unwrap();
                let previous_transactions = eth_ops.into_iter()
                    .filter_map(
                        |eth_op| {
                            if eth_op.tx_hash != current_transaction.tx_hash {
                                Some(eth_op.tx_hash[2..].parse().unwrap())
                            } else {
                                None
                            }
                        }
                    ).collect::<Vec<H256>>();
            }
                (op, OperationState::InProgress { current_transaction, previous_transactions})
        ).collect();
        Ok(())
    }

    fn run(&mut self, rx_for_eth: Receiver<Operation>) {
        loop {
            while let Ok(op) = rx_for_eth.try_recv() {
                self.unsent_ops.push_back(op);
            }

            while self.unconfirmed_ops.len() < MAX_UNCONFIRMED_TX && !self.unsent_ops.is_empty() {
                if let Some(op) = self.unsent_ops.pop_front() {
                    self.unconfirmed_ops
                        .push_back((op, OperationState::Pending));
                }
            }

            for (op, op_state) in self.unconfirmed_ops.iter_mut() {
                if op_state.is_unsent() {
                    match self.drive_to_completion(op, op_state) {
                        Err(e) => {
                            warn!("Error while sending unsent op: {}", e);
                            break;
                        }
                        _ => {}
                    }
                }
            }

            while let Some((op, mut op_state)) = self.unconfirmed_ops.pop_front() {
                self.drive_to_completion(&op, &mut op_state)
                    .map_err(|e| {
                        warn!("Error while trying to complete uncommited op: {}", e);
                    })
                    .unwrap_or_default();

                if !op_state.is_completed() {
                    self.unconfirmed_ops.push_front((op, op_state));
                    break;
                }
            }
        }
    }

    fn commited_nonce(&self) -> Result<u64, failure::Error> {
        Ok(self.eth_client.current_nonce().wait().map(|n| n.as_u64)?)
    }
    fn block_number(&self) -> Result<u64, failure::Error> {
        Ok(self
            .eth_client
            .web3
            .eth()
            .block_number()
            .wait()
            .map(|n| n.as_u64)?)
    }

    fn get_tx_status(&self, hash: &H256) -> Resutl<Option<(u64, u64)>, failure::Error> {
        let block_number = self.block_number()?;
        match self
            .eth_client
            .web3
            .eth()
            .transaction_receipt(*hash)
            .wait()?
        {
            Some(TransactionReceipt {
                block_number: Some(block_number),
                status: Some(status),
                ..
            }) => {
                let confirmations = block_number.saturating_sub(block_number.as_u64());
                Ok(Some((confirmations, status.as_u64())))
            }
            _ => Ok(None),
        }
    }

    fn drive_tx_to_completion(
        &self,
        tx: &SignedCallResult,
        tx_state: &mut SignedTxState,
    ) -> Result<(), failure::Error> {
        match tx_state {
            SignedTxState::Unsent => {
                let deadline_block = self.block_number()? + EXPECTED_WAIT_TIME_BLOCKS;
                *tx_state = SignedTxState::Pending { deadline_block };
                unimplemented!("save to db");
                let hash = self.eth_client.send_raw_tx(tx.raw_tx.clone())?;
            }
            SignedTxState::Pending { .. } => {
                let tx_info = self
                    .eth_client
                    .web3
                    .eth()
                    .transaction(TransactionId::Hash(tx.hash))
                    .wait()?;
                if let Some(Transaction {
                    block_number: Some(block_number),
                    ..
                }) = tx_info
                {
                    // transaction was confirmed, get receipt
                    if let Some((confirmations, status)) = self.get_tx_status(&tx.hash)? {
                        *tx_state = SignedTxState::Executed {
                            confirmations,
                            status,
                        };
                    }
                } else {
                    // transaction is unknown? resend
                    self.eth_client.send_raw_tx(tx.raw_tx.clone())?;
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
                    *tx_state = SignedTxState::Executed {
                        confirmations: *confirmations,
                        status: *status,
                    }
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
            }
            OperationState::InProgress {
                current_transaction: (tx, tx_state),
                previous_transactions,
            } => {
                let current_nonce = self.commited_nonce()?;

                self.drive_tx_to_completion(tx, tx_state)?;

                if tx_state.stuck() {
                    let new_tx = self.resign_op_tx(op, tx)?;
                    unimplemented!("persist to db");
                    previous_transactions.push(tx.hash);
                    *tx = new_tx;
                    *tx_state = SignedTxState::Unsent;
                    self.drive_tx_to_completion(tx, tx_state)?;
                }

                if let Some(status) = tx_state.confirmed_status() {
                    let success = status == 1;
                    unimplemented!("save commited tx to db");
                    *state = OperationState::Commited {
                        success,
                        hash: tx.hash,
                    };
                } else if current_nonce > tx.nonce.as_u64() {
                    // some older tx was commited.
                    for old_tx in previous_transactions {
                        if let Some((confirmations, status)) = self.get_tx_status(old_tx)? {
                            if confirmations >= WAIT_CONFIRMATIONS {
                                let success = status == 1;
                                unimplemented!("save commited tx to db") * state =
                                    OperationState::Commited {
                                        success,
                                        hash: *old_tx,
                                    };
                            }
                        }
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
            // replacement price should be at least 10% higher.
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
