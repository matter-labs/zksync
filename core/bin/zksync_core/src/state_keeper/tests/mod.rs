use super::{ZkSyncStateInitParams, ZkSyncStateKeeper};
use futures::channel::mpsc;
use zksync_types::{AccountId, H160, *};

mod apply_priority_op;
mod apply_tx;
mod execute_proposed_block;
mod gas_limit;
mod pending_block;
mod utils;

/// Checks that StateKeeper will panic with incorrect initialization data
#[test]
#[should_panic]
fn test_create_incorrect_state_keeper() {
    const CHANNEL_SIZE: usize = 32768;
    const MAX_ITERATIONS: usize = 100;
    const FAST_ITERATIONS: usize = 100;

    let (events_sender, _events_receiver) = mpsc::channel(CHANNEL_SIZE);
    let (_request_tx, request_rx) = mpsc::channel(CHANNEL_SIZE);
    let (response_tx, _response_rx) = mpsc::channel(CHANNEL_SIZE);

    let fee_collector = Account::default_with_address(&H160::random());

    let mut init_params = ZkSyncStateInitParams::default();
    init_params.insert_account(AccountId(0), fee_collector.clone());

    // should panic
    ZkSyncStateKeeper::new(
        init_params,
        fee_collector.address,
        request_rx,
        response_tx,
        vec![1, 2, 2], // `available_block_chunk_sizes` must be strictly increasing.
        MAX_ITERATIONS,
        FAST_ITERATIONS,
        events_sender,
    );
}
