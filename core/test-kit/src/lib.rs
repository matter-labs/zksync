use futures::channel::mpsc;
use server::state_keeper::{start_state_keeper, PlasmaStateKeeper};
use storage::ConnectionPool;
use tokio::runtime::Runtime;

fn init_and_run_state_keeper() {
    let mut main_runtime = Runtime::new().expect("main runtime start");
    let connection_pool = ConnectionPool::new();

    //    let (proposed_blocks_sender, proposed_blocks_receiver) = mpsc::channel(256);
    //    let (state_keeper_req_sender, state_keeper_req_receiver) = mpsc::channel(256);
    //    let (executed_tx_notify_sender, executed_tx_notify_receiver) = mpsc::channel(256);

    //    let state_keeper = PlasmaStateKeeper::new(
    //        connection_pool.clone(),
    //        config_opts.operator_franklin_addr.clone(),
    //        state_keeper_req_receiver,
    //        proposed_blocks_sender,
    //        executed_tx_notify_sender,
    //    );
    //    start_state_keeper(state_keeper, &main_runtime);
}
