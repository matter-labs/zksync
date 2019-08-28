//! Transaction processing policy.
//! only one pending tx at a time.
//! poll for tx execu
//!

use eth_client::ETHClient;
use ff::{PrimeField, PrimeFieldRepr};
use models::abi::TEST_PLASMA2_ALWAYS_VERIFY;
use models::{Action, Operation};
use std::collections::VecDeque;
use std::str::FromStr;
use std::sync::mpsc::{channel, Receiver, Sender};
use storage::ConnectionPool;
use web3::types::{H256, U256};

struct ETHSender {
    eth_client: ETHClient,
    op_queue: VecDeque<Operation>,
    pending_transaction: Option<H256>,
    db_pool: ConnectionPool,
}

impl ETHSender {
    fn new(db_pool: ConnectionPool) -> Self {
        let eth_client = {
            let abi_string = serde_json::Value::from_str(TEST_PLASMA2_ALWAYS_VERIFY)
                .unwrap()
                .get("abi")
                .unwrap()
                .to_string();
            ETHClient::new(abi_string)
        };
        let mut sender = Self {
            eth_client,
            op_queue: VecDeque::new(),
            pending_transaction: None,
            db_pool,
        };
        sender.restore_state();
        sender
    }

    fn restore_state(&mut self) {
        let current_nonce = self
            .eth_client
            .current_nonce()
            .expect("could not fetch current nonce");
        let storage = self.db_pool.access_storage().expect("db fail");
        info!(
            "Starting eth_sender: sender = {}, current_nonce = {}",
            self.eth_client.current_sender(),
            current_nonce
        );
        // execute pending transactions
        let ops = storage
            .load_unsent_ops(current_nonce)
            .expect("db must be functional");
        for pending_op in ops {
            self.op_queue.push_back(pending_op);
        }

        let commited_nonce = self.eth_client.current_nonce().expect("eth nonce");
        let pending_nonce = self.eth_client.pending_nonce().expect("eth pending nonce");
        if commited_nonce == pending_nonce {
            self.pending_transaction = None;
        } else if commited_nonce + 1 == pending_nonce{
            let last_sent_op = storage.load_last_sent_operation().expect("db error");
            if let Some(op) = last_sent_op {
                assert_eq!(op.nonce, pending_nonce as i64);
                self.pending_transaction = Some(op.tx_hash.unwrap()[2..].parse());
            } else {
                self.pending_transaction = None;
            }
        } else {
            panic!("Only one transaction can be pending at once.");
        }
    }

    fn run(&mut self, rx_for_eth: Receiver<Operation>) {
        // check pending tx
        // success? -> pop next operation
        // timeout? -> resend with higher gas price
        // fail? -> notify state_keeper.
        let storage = self
            .db_pool
            .access_storage()
            .expect("db connection failed for eth sender");

        loop {
            let new_op = rx_for_eth.recv_timeout(std::time::Duration::from_secs(5));
            match new_op {
                Ok(op) => self.op_queue.push_back(op),
                _ => (),
            }

            if let Some(pending_tx) = self.pending_transaction.take() {
                // check tx status.
                unimplemented!();
            } else {
                if let Some(new_op) = self.op_queue.pop_front() {
                    let tx_hash = self.send_operation(&new_op);
                    match tx_hash {
                        Ok(hash) => {
                            debug!("Commitment tx hash = {:?}", hash);
                            storage
                                .save_operation_tx_hash(
                                    new_op.id.expect("trying to send not stored op?"),
                                    format!("{:?}", hash),
                                )
                                .expect("Failed to save tx hash to db");
                            self.pending_transaction = Some(hash);
                        }
                        Err(err) => {
                            error!("Error sending tx {}", err);
                            self.op_queue.push_front(new_op);
                        }
                    }
                }
            }
        }
    }

    fn send_operation(&mut self, op: &Operation) -> eth_client::Result<H256> {
        match &op.action {
            Action::Commit => {
                let mut be_bytes: Vec<u8> = Vec::new();
                op.block
                    .new_root_hash
                    .into_repr()
                    .write_be(&mut be_bytes)
                    .expect("Write commit bytes");
                let root = H256::from(U256::from_big_endian(&be_bytes));
                debug!(
                    "public_data for block_number {}: {:x?}",
                    op.block.block_number,
                    op.block.get_eth_public_data()
                );
                // function commitBlock(uint32 _blockNumber, uint24 _feeAccount, bytes32 _newRoot, bytes calldata _publicData)
                self.eth_client.call(
                    "commitBlock",
                    op.tx_meta.clone().expect("tx meta missing"),
                    (
                        u64::from(op.block.block_number),
                        u64::from(op.block.fee_account),
                        root,
                        op.block.get_eth_public_data(),
                    ),
                )
            }
            Action::Verify { proof } => {
                // function verifyBlock(uint32 _blockNumber, uint256[8] calldata proof) external {
                self.eth_client.call(
                    "verifyBlock",
                    op.tx_meta.clone().expect("tx meta missing"),
                    (u64::from(op.block.block_number), *proof.clone()),
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
            let mut eth_sender = ETHSender::new(pool);
            eth_sender.run(rx_for_eth);
        })
        .expect("Eth sender thread");

    tx_for_eth
}
