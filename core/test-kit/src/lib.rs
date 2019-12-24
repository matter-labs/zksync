use log::*;

use bigdecimal::BigDecimal;
use franklin_crypto::eddsa::{PrivateKey, PublicKey, Signature};
use futures::{channel::mpsc, SinkExt, StreamExt};
use models::node::tx::TxSignature;
use models::node::{Account, AccountAddress, AccountMap, Engine, FranklinTx, Transfer};
use rand::{Rng, SeedableRng, XorShiftRng};
use server::mempool::ProposedBlock;
use server::state_keeper::{
    start_state_keeper, PlasmaStateInitParams, PlasmaStateKeeper, StateKeeperRequest,
};
use server::ConfigurationOptions;
use storage::ConnectionPool;
use tokio::runtime::Runtime;

struct TestBlock {
    transactions: Vec<FranklinTx>,
}

fn gen_pk() -> PrivateKey<Engine> {
    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);

    PrivateKey(rng.gen())
}

fn new_transfer() -> FranklinTx {
    let pk = gen_pk();
    let mut transfer = Transfer {
        from: AccountAddress::default(),
        to: AccountAddress::default(),
        token: 0,
        amount: BigDecimal::from(0),
        fee: BigDecimal::from(0),
        nonce: 0,
        signature: TxSignature::default(),
    };
    transfer.signature = TxSignature::sign_musig_pedersen(&pk, &transfer.get_bytes());
    FranklinTx::Transfer(transfer)
}

fn genesis_state(config_opts: &ConfigurationOptions) -> PlasmaStateInitParams {
    let mut accounts = AccountMap::default();
    let operator_account = Account::default_with_address(&config_opts.operator_franklin_addr);
    accounts.insert(0, operator_account);

    PlasmaStateInitParams {
        accounts,
        last_block_number: 0,
        unprocessed_priority_op: 0,
    }
}

fn dummy_proposed_block() -> ProposedBlock {
    ProposedBlock {
        priority_ops: Vec::new(),
        txs: vec![new_transfer()],
    }
}

pub fn init_and_run_state_keeper() {
    let mut main_runtime = Runtime::new().expect("main runtime start");
    let connection_pool = ConnectionPool::new();
    let config = ConfigurationOptions::from_env();

    let (proposed_blocks_sender, mut proposed_blocks_receiver) = mpsc::channel(256);
    let (state_keeper_req_sender, state_keeper_req_receiver) = mpsc::channel(256);
    let (executed_tx_notify_sender, executed_tx_notify_receiver) = mpsc::channel(256);

    let state_keeper = PlasmaStateKeeper::new(
        genesis_state(&config),
        config.operator_franklin_addr.clone(),
        state_keeper_req_receiver,
        proposed_blocks_sender,
        executed_tx_notify_sender,
    );
    start_state_keeper(state_keeper, &main_runtime);

    let empty_block = async move {
        println!("go1");
        state_keeper_req_sender
            .clone()
            .send(StateKeeperRequest::ExecuteMiniBlock(dummy_proposed_block()))
            .await;
        println!("go2");
        state_keeper_req_sender
            .clone()
            .send(StateKeeperRequest::SealBlock)
            .await;
        println!("go3");
        println!("go1");
        state_keeper_req_sender
            .clone()
            .send(StateKeeperRequest::ExecuteMiniBlock(dummy_proposed_block()))
            .await;
        println!("go2");
        state_keeper_req_sender
            .clone()
            .send(StateKeeperRequest::SealBlock)
            .await;
        println!("go3");
        state_keeper_req_sender
            .clone()
            .send(StateKeeperRequest::SealBlock)
            .await;
        println!("go3");
        state_keeper_req_sender
            .clone()
            .send(StateKeeperRequest::SealBlock)
            .await;
        println!("go3");
    };

    main_runtime.block_on(async move {
        empty_block.await;
        while let Some(op) = proposed_blocks_receiver.next().await {
            println!("op: {:#?}", op);
        }
    });
}
