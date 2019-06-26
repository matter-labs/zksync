use circuit::encoder;
use config;
use eth_client::ETHClient;
use ff::{PrimeField, PrimeFieldRepr};
use models::abi::TEST_PLASMA_ALWAYS_VERIFY;
use models::plasma::block::BlockData;
use models::plasma::{params, AccountMap, AccountUpdate};
use models::*;
use std::sync::mpsc::{channel, Receiver, Sender};
use storage::ConnectionPool;
use web3::types::{H256, U128, U256};

fn sorted_and_padded_for_deposits(
    accounts_updated: &[AccountUpdate],
) -> [u64; config::DEPOSIT_BATCH_SIZE] {
    assert_eq!(accounts_updated.len(), config::DEPOSIT_BATCH_SIZE);

    let mut tmp = [u64::from(params::SPECIAL_ACCOUNT_DEPOSIT); config::DEPOSIT_BATCH_SIZE];
    let mut acc: Vec<u64> = accounts_updated
        .iter()
        .map(|acc| u64::from(acc.get_account_id()))
        .collect();
    acc.sort();

    for (i, a) in acc.into_iter().enumerate() {
        tmp[i] = a;
    }

    tmp
}

fn sorted_and_padded_for_exits(
    accounts_updated: &[AccountUpdate],
) -> [u64; config::EXIT_BATCH_SIZE] {
    assert_eq!(accounts_updated.len(), config::EXIT_BATCH_SIZE);

    let mut tmp = [u64::from(params::SPECIAL_ACCOUNT_EXIT); config::EXIT_BATCH_SIZE];
    let mut acc: Vec<u64> = accounts_updated
        .iter()
        .map(|acc| u64::from(acc.get_account_id()))
        .collect();
    acc.sort();

    for (i, a) in acc.into_iter().enumerate() {
        tmp[i] = a;
    }

    tmp
}

#[allow(dead_code)]
fn keys_sorted(accounts_updated: AccountMap) -> Vec<u64> {
    let mut acc: Vec<u64> = accounts_updated.keys().map(|&k| u64::from(k)).collect();
    acc.sort();
    acc
}

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

                match &op.block.block_data {
                    BlockData::Transfer { total_fees, .. } => {
                        // let eth_block_data = EthBlockData::Transfer{
                        //     total_fees:     U128::from_dec_str(&total_fees.to_string()).expect("fee should fit into U128 Ethereum type"),
                        //     public_data:    encoder::encode_transfer_transactions(&block).unwrap(),
                        // };

                        let total_fees = U128::from_dec_str(&total_fees.to_string())
                            .expect("fee should fit into U128 Ethereum type");
                        let public_data = encoder::encode_transactions(&op.block).unwrap();

                        // let mut be_bytes: Vec<u8> = vec![];
                        // &block.new_root_hash.clone().into_repr().write_be(&mut be_bytes);
                        // let root = H256::from(U256::from_big_endian(&be_bytes));

                        eth_client.call(
                            "commitTransferBlock",
                            op.tx_meta.expect("tx meta missing"),
                            (
                                u64::from(op.block.block_number),
                                total_fees,
                                public_data,
                                root,
                            ),
                        )
                    }

                    BlockData::Deposit { batch_number, .. } => {
                        // let eth_block_data = EthBlockData::Deposit{ batch_number };
                        // let mut be_bytes: Vec<u8> = vec![];
                        // &block.new_root_hash.clone().into_repr().write_be(& mut be_bytes);
                        // let root = H256:

                        eth_client.call(
                            "commitDepositBlock",
                            op.tx_meta.expect("tx meta missing"),
                            (
                                U256::from(*batch_number),
                                sorted_and_padded_for_deposits(&op.accounts_updated.unwrap()),
                                u64::from(op.block.block_number),
                                root,
                            ),
                        )
                    }

                    BlockData::Exit { batch_number, .. } => {
                        // let eth_block_data = EthBlockData::Exit{
                        //     batch_number,
                        //     public_data: encoder::encode_exit_transactions(&block).expect("must encode exit block information")
                        // };
                        // let mut be_bytes: Vec<u8> = vec![];
                        // &block.new_root_hash.clone().into_repr().write_be(& mut be_bytes);
                        // let root = H256::fro

                        let public_data = encoder::encode_transactions(&op.block).unwrap();
                        eth_client.call(
                            "commitExitBlock",
                            op.tx_meta.expect("tx meta missing"),
                            (
                                U256::from(*batch_number),
                                sorted_and_padded_for_exits(&op.accounts_updated.unwrap()),
                                u64::from(op.block.block_number),
                                public_data,
                                root,
                            ),
                        )
                    }
                }
            }
            Action::Verify { proof } => match op.block.block_data {
                BlockData::Transfer { .. } => eth_client.call(
                    "verifyTransferBlock",
                    op.tx_meta.expect("tx meta missing"),
                    (u64::from(op.block.block_number), *proof),
                ),

                BlockData::Deposit { batch_number, .. } => eth_client.call(
                    "verifyDepositBlock",
                    op.tx_meta.expect("tx meta missing"),
                    (
                        U256::from(batch_number),
                        sorted_and_padded_for_deposits(&op.accounts_updated.unwrap()),
                        u64::from(op.block.block_number),
                        *proof,
                    ),
                ),

                BlockData::Exit { batch_number, .. } => eth_client.call(
                    "verifyExitBlock",
                    op.tx_meta.expect("tx meta missing"),
                    (
                        u64::from(batch_number),
                        u64::from(op.block.block_number),
                        *proof,
                    ),
                ),
            },
        };
        // TODO: process tx sending failure
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
