use crate::eth_watch::client::EthClient;
use crate::eth_watch::storage::Storage;
use crate::eth_watch::{EthWatch, EthWatchRequest};
use futures::channel::mpsc;
use futures::channel::mpsc::Sender;
use web3::types::{Address, BlockNumber};
use zksync_types::ethereum::CompleteWithdrawalsTx;
use zksync_types::PriorityOp;

struct FakeStorage {
    withdrawal_txs: Vec<CompleteWithdrawalsTx>,
}
impl FakeStorage {
    fn new() -> Self {
        Self {
            withdrawal_txs: vec![],
        }
    }
}

impl Storage for FakeStorage {
    async fn store_complete_withdrawals(
        &mut self,
        complete_withdrawals_txs: Vec<CompleteWithdrawalsTx>,
    ) -> anyhow::Result<()> {
        self.withdrawal_txs.extend(complete_withdrawals_txs);
        Ok(())
    }
}

struct FakeEthWorker {}
impl FakeEthWorker {
    fn new() -> Self {
        Self {}
    }
}

impl EthClient for FakeEthWorker {
    async fn get_priority_op_events(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<PriorityOp>, anyhow::Error> {
        unimplemented!()
    }

    async fn get_complete_withdrawals_event(
        &self,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Result<Vec<CompleteWithdrawalsTx>, anyhow::Error> {
        unimplemented!()
    }

    async fn block_number(&self) -> Result<u64, anyhow::Error> {
        unimplemented!()
    }

    async fn get_auth_fact(&self, address: Address, nonce: u32) -> Result<Vec<u8>, anyhow::Error> {
        unimplemented!()
    }

    async fn get_first_pending_withdrawal_index(&self) -> Result<u32, anyhow::Error> {
        unimplemented!()
    }

    async fn get_number_of_pending_withdrawals(&self) -> Result<u32, anyhow::Error> {
        unimplemented!()
    }
}

fn create_watcher(
    client: FakeEthWorker,
) -> (
    EthWatch<FakeEthWorker, FakeStorage>,
    Sender<EthWatchRequest>,
) {
    let storage = FakeStorage::new();
    let (eth_watch_req_sender, eth_watch_req_receiver) = mpsc::channel(10);
    let eth_watch = EthWatch::new(client, storage, 2, eth_watch_req_receiver);

    (eth_watch, eth_watch_req_sender)
}

#[test]
fn test() {
    let client = FakeEthWorker::new();
    let (watcher, sender) = create_watcher(client);
}
