use config;
use eth_client::ETHClient;
use ff::{PrimeField, PrimeFieldRepr};
use models::abi::TEST_PLASMA_ALWAYS_VERIFY;
use models::plasma::tx::FranklinTx;
use models::plasma::{params, AccountMap, AccountUpdate};
use models::*;
use std::sync::mpsc::{channel, Receiver, Sender};
use storage::ConnectionPool;
use web3::types::{H256, U128, U256};

fn run_eth_sender(
    pool: ConnectionPool,
    rx_for_eth: Receiver<Operation>,
    mut eth_client: ETHClient,
) {
    let storage = pool
        .access_storage()
        .expect("db connection failed for eth sender");
    for op in rx_for_eth {
        //debug!("Operation requested");
        debug!(
            "Operation requested: {:?}, {}",
            &op.action, op.block.block_number
        );
        let tx = match op.action {
            Action::Commit => {
                let mut be_bytes: Vec<u8> = Vec::new();
                op.block
                    .new_root_hash
                    .into_repr()
                    .write_be(&mut be_bytes)
                    .expect("Write commit bytes");
                let root = H256::from(U256::from_big_endian(&be_bytes));

                // function commitBlock(uint32 _blockNumber, bytes32 _newRoot, bytes calldata _publicData) external {
                eth_client.call(
                    "commitBlock",
                    op.tx_meta.expect("tx meta missing"),
                    (
                        u64::from(op.block.block_number),
                        root,
                        op.block.get_eth_public_data(),
                    ),
                )
            }
            Action::Verify { proof } => {
                //            function verifyBlock(uint32 _blockNumber, uint256[8] calldata proof) external {
                eth_client.call(
                    "verifyBlock",
                    op.tx_meta.expect("tx meta missing"),
                    (u64::from(op.block.block_number), *proof),
                )
            }
        };
        // TODO: process tx sending failure
        // proposal - if there is gas problems - retry with new gas price/gas volume according to policy
        // if there is tx fail -- propogate error to state keeper.
        match tx {
            Ok(hash) => {
                debug!("Commitment tx hash = {:?}", hash);
                let _ = storage.save_operation_tx_hash(
                    op.id.expect("trying to send not stored op?"),
                    format!("{:?}", hash),
                );
            }
            Err(err) => error!("Error sending tx {}", err),
        }
    }
}

pub fn start_eth_sender(pool: ConnectionPool) -> Sender<Operation> {
    let (tx_for_eth, rx_for_eth) = channel::<Operation>();
    let eth_client = ETHClient::new(TEST_PLASMA_ALWAYS_VERIFY);
    let storage = pool
        .access_storage()
        .expect("db connection failed for eth sender");
    let current_nonce = eth_client
        .current_nonce()
        .expect("could not fetch current nonce");
    info!(
        "Starting eth_sender: sender = {}, current_nonce = {}",
        eth_client.current_sender(),
        current_nonce
    );

    // execute pending transactions
    let ops = storage
        .load_unsent_ops(current_nonce)
        .expect("db must be functional");
    for pending_op in ops {
        tx_for_eth
            .send(pending_op)
            .expect("must send a request for ethereum transaction for pending operations");
    }

    std::thread::Builder::new()
        .name("eth_sender".to_string())
        .spawn(move || {
            run_eth_sender(pool, rx_for_eth, eth_client);
        })
        .expect("Eth sender thread");

    tx_for_eth
}
