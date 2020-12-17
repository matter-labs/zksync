use zksync_storage::ConnectionPool;
use zksync_types::ethereum::CompleteWithdrawalsTx;

#[async_trait::async_trait]
pub trait Storage {
    async fn store_complete_withdrawals(
        &mut self,
        complete_withdrawals_txs: Vec<CompleteWithdrawalsTx>,
    ) -> anyhow::Result<()>;
}

pub struct DBStorage {
    db_pool: ConnectionPool,
}

impl DBStorage {
    pub fn new(db_pool: ConnectionPool) -> Self {
        Self { db_pool }
    }
}

#[async_trait::async_trait]
impl Storage for DBStorage {
    async fn store_complete_withdrawals(
        &mut self,
        complete_withdrawals_txs: Vec<CompleteWithdrawalsTx>,
    ) -> anyhow::Result<()> {
        unreachable!()
    }
}
