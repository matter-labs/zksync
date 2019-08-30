//! Transaction processing policy.
//! only one pending tx at a time.
//! poll for tx execu
//!

use bigdecimal::BigDecimal;
use eth_client::{CallResult, ETHClient};
use failure::format_err;
use ff::{PrimeField, PrimeFieldRepr};
use futures::Future;
use models::abi::TEST_PLASMA2_ALWAYS_VERIFY;
use models::{Action, Operation};
use num_traits::cast::FromPrimitive;
use std::collections::VecDeque;
use std::str::FromStr;
use std::sync::mpsc::{channel, Receiver, Sender};
use storage::ConnectionPool;
use web3::contract::Options;
use web3::transports::Http;
use web3::types::H256;
use web3::Transport;

const EXPECTED_WAIT_TIME_BLOCKS: usize = 10;
const MAX_UNCONFIRMED_TX: usize = 5;

struct ETHSender<T: Transport> {
    eth_client: ETHClient<T>,
    unsent_ops: VecDeque<Operation>,
    unconfirmed_ops: VecDeque<(H256, Operation)>,
    //(tx hash, operation)
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
            .map(|(op, eth_op)| (eth_op.tx_hash[2..].parse().unwrap(), op))
            .collect();

        Ok(())
    }

    fn run(&mut self, rx_for_eth: Receiver<Operation>) {
        let storage = self.db_pool.access_storage().expect("db access");
        loop {
            while let Ok(op) = rx_for_eth.try_recv() {
                self.unsent_ops.push_back(op);
            }

            while self.unconfirmed_ops.len() < MAX_UNCONFIRMED_TX && !self.unsent_ops.is_empty() {
                if let Some(op) = self.unsent_ops.pop_front() {
                    match self.send_operation(&op) {
                        Ok(res) => {
                            info!(
                                "Operation {} sent, tx_hash: {:#x}, gas_price: {}, nonce: {}",
                                op.id.unwrap(),
                                res.hash,
                                res.gas_price,
                                res.nonce
                            );
                            storage
                                .save_operation_eth_tx(
                                    op.id.unwrap(),
                                    res.hash,
                                    res.nonce.as_u32(),
                                    BigDecimal::from_u128(res.gas_price.as_u128()).unwrap(),
                                )
                                .expect("db fail");
                            self.unconfirmed_ops.push_back((res.hash, op));
                        }
                        Err(e) => {
                            error!("Failed to send eth tx: {}", e);
                            self.unsent_ops.push_front(op);
                            break;
                        }
                    }
                }
            }

            // check pending txs
            while let Some((hash, op)) = self.unconfirmed_ops.front() {
                match self.eth_client.web3.eth().transaction_receipt(*hash).wait() {
                    Ok(tx_receipt) => {
                        if let Some(tx_receipt) = tx_receipt {
                            if let Some(status) = tx_receipt.status {
                                info!(
                                    "Operation {} confirmed, tx_hash: {:#x}, gas_used: {:?}",
                                    op.id.unwrap(),
                                    hash,
                                    tx_receipt.gas_used
                                );
                                storage.confirm_eth_tx(hash).expect("db fail");
                                if status.as_u64() != 1 {
                                    error!(
                                        "Operation {} failed, tx_hash: {:#x}",
                                        op.id.unwrap(),
                                        hash
                                    )
                                }
                                self.unconfirmed_ops.pop_front();
                            }
                        }
                    }
                    Err(e) => {
                        error!("Error while checking pending transaction: {}", e);
                        break;
                    }
                }
            }

            std::thread::sleep(std::time::Duration::from_secs(5));
        }
    }

    fn send_operation(&mut self, op: &Operation) -> Result<CallResult, web3::Error> {
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
                self.eth_client
                    .call(
                        "commitBlock",
                        (
                            u64::from(op.block.block_number),
                            u64::from(op.block.fee_account),
                            root,
                            op.block.get_eth_public_data(),
                        ),
                        Options::default(),
                    )
                    .wait()
            }
            Action::Verify { proof } => {
                // function verifyBlock(uint32 _blockNumber, uint256[8] calldata proof) external {
                self.eth_client
                    .call(
                        "verifyBlock",
                        (u64::from(op.block.block_number), *proof.clone()),
                        Options::default(),
                    )
                    .wait()
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
