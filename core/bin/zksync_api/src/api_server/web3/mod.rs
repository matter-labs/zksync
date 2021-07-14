// Built-in uses
use std::convert::TryInto;
// External uses
use futures::channel::mpsc;
use jsonrpc_core::{Error, IoHandler, MetaIoHandler, Metadata, Middleware, Result};
use jsonrpc_http_server::ServerBuilder;
// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_crypto::convert::FeConvert;
use zksync_storage::{ConnectionPool, StorageProcessor};
use zksync_types::ExecutedOperations;
use zksync_utils::panic_notify::ThreadPanicNotify;
// Local uses
use self::{
    rpc_trait::Web3Rpc,
    types::{BlockInfo, Transaction, TxData, H256, U256},
};

mod rpc_impl;
mod rpc_trait;
mod types;

#[derive(Clone)]
pub struct Web3RpcApp {
    runtime_handle: tokio::runtime::Handle,
    connection_pool: ConnectionPool,
    chain_id: u8,
}

impl Web3RpcApp {
    pub fn new(connection_pool: ConnectionPool, chain_id: u8) -> Self {
        let runtime_handle = tokio::runtime::Handle::try_current()
            .expect("Web3RpcApp must be created from the context of Tokio Runtime");

        Web3RpcApp {
            runtime_handle,
            connection_pool,
            chain_id,
        }
    }

    pub fn extend<T: Metadata, S: Middleware<T>>(self, io: &mut MetaIoHandler<T, S>) {
        io.extend_with(self.to_delegate())
    }
}

impl Web3RpcApp {
    async fn access_storage(&self) -> Result<StorageProcessor<'_>> {
        self.connection_pool
            .access_storage()
            .await
            .map_err(|_| Error::internal_error())
    }

    async fn resolve_block_number(
        storage: &mut StorageProcessor<'_>,
        number: Option<self::types::BlockNumber>,
    ) -> Result<Option<zksync_types::BlockNumber>> {
        let last_saved_block = storage
            .chain()
            .block_schema()
            .get_last_saved_block()
            .await
            .map_err(|_| Error::internal_error())?;

        let number = match number {
            Some(number) => number,
            None => {
                return Ok(Some(last_saved_block));
            }
        };

        let number = match number {
            self::types::BlockNumber::Earliest => zksync_types::BlockNumber(0),
            self::types::BlockNumber::Committed => storage
                .chain()
                .block_schema()
                .get_last_committed_confirmed_block()
                .await
                .map_err(|_| Error::internal_error())?,
            self::types::BlockNumber::Finalized => storage
                .chain()
                .block_schema()
                .get_last_verified_confirmed_block()
                .await
                .map_err(|_| Error::internal_error())?,
            self::types::BlockNumber::Latest | self::types::BlockNumber::Pending => {
                last_saved_block
            }
            self::types::BlockNumber::Number(number) => {
                if number.as_u64() > last_saved_block.0 as u64 {
                    return Ok(None);
                }
                // Unwrap can be safely used because `number` is not greater than `last_saved_block`
                // which is `u32` variable.
                zksync_types::BlockNumber(number.as_u64().try_into().unwrap())
            }
        };
        Ok(Some(number))
    }

    async fn block_transaction_count(
        storage: &mut StorageProcessor<'_>,
        block_number: zksync_types::BlockNumber,
    ) -> Result<U256> {
        let count = storage
            .chain()
            .block_schema()
            .get_block_transactions_count(block_number)
            .await
            .map_err(|_| Error::internal_error())?;
        Ok(U256::from(count))
    }

    fn transaction_from_tx_data(tx: TxData) -> Transaction {
        Transaction {
            hash: tx.tx_hash,
            nonce: tx.nonce.into(),
            block_hash: tx.block_hash,
            block_number: tx.block_number.map(Into::into),
            transaction_index: tx.block_index.map(Into::into),
            from: tx.from,
            to: tx.to,
            value: 0.into(),
            gas_price: 0.into(),
            gas: 0.into(),
            input: Vec::new().into(),
            raw: None,
        }
    }

    async fn block_by_number(
        storage: &mut StorageProcessor<'_>,
        block_number: zksync_types::BlockNumber,
        include_txs: bool,
    ) -> Result<BlockInfo> {
        let parent_hash = if block_number.0 == 0 {
            H256::zero()
        } else {
            // It was already checked that the block is in storage, so the parent block has to be there too.
            let parent_block = storage
                .chain()
                .block_schema()
                .get_storage_block(block_number - 1)
                .await
                .map_err(|_| Error::internal_error())?
                .expect("Can't find parent block in storage");
            H256::from_slice(&parent_block.root_hash)
        };

        if include_txs {
            // It was already checked that the block is in storage.
            let block = storage
                .chain()
                .block_schema()
                .get_block(block_number)
                .await
                .map_err(|_| Error::internal_error())?
                .expect("Can't find block in storage");
            let hash = H256::from_slice(&block.new_root_hash.to_bytes());
            let transactions = block
                .block_transactions
                .into_iter()
                .map(|tx| {
                    let tx = match tx {
                        ExecutedOperations::Tx(tx) => TxData {
                            block_hash: Some(hash),
                            block_number: Some(block_number.0),
                            block_index: tx.block_index,
                            from: tx.signed_tx.tx.from_account(),
                            to: tx.signed_tx.tx.to_account(),
                            nonce: tx.signed_tx.tx.nonce().0,
                            tx_hash: H256::from_slice(tx.signed_tx.tx.hash().as_ref()),
                        },
                        ExecutedOperations::PriorityOp(op) => TxData {
                            block_hash: Some(hash),
                            block_number: Some(block_number.0),
                            block_index: Some(op.block_index),
                            from: op.priority_op.data.from_account(),
                            to: op.priority_op.data.to_account(),
                            nonce: op.priority_op.serial_id as u32,
                            tx_hash: H256::from_slice(op.priority_op.tx_hash().as_ref()),
                        },
                    };
                    Self::transaction_from_tx_data(tx)
                })
                .collect();

            Ok(BlockInfo::new_with_txs(
                hash,
                parent_hash,
                block_number,
                block.timestamp,
                transactions,
            ))
        } else {
            // It was already checked that the block is in storage.
            let block = storage
                .chain()
                .block_schema()
                .get_storage_block(block_number)
                .await
                .map_err(|_| Error::internal_error())?
                .expect("Can't find block in storage");
            let hash = H256::from_slice(&block.root_hash);
            let transactions = storage
                .chain()
                .block_schema()
                .get_block_transactions_hashes(block_number)
                .await
                .map_err(|_| Error::internal_error())?
                .into_iter()
                .map(|hash| H256::from_slice(&hash))
                .collect();

            Ok(BlockInfo::new_with_hashes(
                hash,
                parent_hash,
                block_number,
                block.timestamp.unwrap_or_default() as u64,
                transactions,
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn start_rpc_server(
    connection_pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
    config: &ZkSyncConfig,
) {
    let addr = config.api.web3.bind_addr();

    let rpc_app = Web3RpcApp::new(connection_pool, config.eth_client.chain_id);
    std::thread::spawn(move || {
        let _panic_sentinel = ThreadPanicNotify(panic_notify);
        let mut io = IoHandler::new();
        rpc_app.extend(&mut io);

        let server = ServerBuilder::new(io)
            .threads(super::THREADS_PER_SERVER)
            .start_http(&addr)
            .unwrap();
        server.wait();
    });
}
