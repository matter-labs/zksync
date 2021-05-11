// Built-in deps
use std::time::Duration;

// External uses
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use tokio::task::JoinHandle;
// Workspace uses
use zksync_config::{TokenHandlerConfig, ZkSyncConfig};
use zksync_storage::{ConnectionPool, StorageProcessor};
use zksync_types::RegisterNFTFactoryEvent;
// Local uses
use crate::eth_watch::EthWatchRequest;

/// Handle events about registering factories for minting tokens
#[derive(Debug)]
struct NFTFactoryHandler {
    connection_pool: ConnectionPool,
    poll_interval: Duration,
    eth_watch_req: mpsc::Sender<EthWatchRequest>,
    last_eth_block: Option<u64>,
}

impl NFTFactoryHandler {
    async fn new(
        connection_pool: ConnectionPool,
        eth_watch_req: mpsc::Sender<EthWatchRequest>,
        config: TokenHandlerConfig,
    ) -> Self {
        let poll_interval = config.poll_interval();

        Self {
            connection_pool,
            eth_watch_req,
            poll_interval,
            last_eth_block: None,
        }
    }

    async fn load_register_nft_factory_events(&self) -> Vec<RegisterNFTFactoryEvent> {
        let (sender, receiver) = oneshot::channel();
        self.eth_watch_req
            .clone()
            .send(EthWatchRequest::GetRegisterNFTFactoryEvents {
                last_eth_block: self.last_eth_block,
                resp: sender,
            })
            .await
            .expect("ETH watch req receiver dropped");

        receiver.await.expect("Err response from eth watch")
    }

    async fn save_register_factory(
        &self,
        storage: &mut StorageProcessor<'_>,
        register_nft_factory_events: Vec<RegisterNFTFactoryEvent>,
    ) -> anyhow::Result<()> {
        let mut transaction = storage.start_transaction().await?;

        let factories = {
            let mut factories = vec![];
            let mut account_schema = transaction.chain().account_schema();
            for factory in register_nft_factory_events {
                // If account does not exists skip factory
                if let Some(account_id) = account_schema
                    .account_id_by_address(factory.creator_address)
                    .await?
                {
                    factories.push((account_id, factory))
                } else {
                    vlog::warn!(
                        "Cant register factory, creator {:?} does not exist",
                        &factory.creator_address
                    )
                }
            }
            factories
        };

        let mut token_schema = transaction.tokens_schema();
        for (account_id, nft_factory) in factories {
            token_schema
                .store_nft_factory(
                    account_id,
                    nft_factory.creator_address,
                    nft_factory.factory_address,
                )
                .await?
        }
        transaction.commit().await?;
        Ok(())
    }

    async fn run(&mut self) {
        let mut timer = tokio::time::interval(self.poll_interval);
        loop {
            timer.tick().await;

            let register_nft_factory_events = self.load_register_nft_factory_events().await;

            self.last_eth_block = register_nft_factory_events
                .iter()
                .map(|event| event.eth_block)
                .max()
                .or(self.last_eth_block);

            let mut storage = self
                .connection_pool
                .access_storage()
                .await
                .expect("db connection failed for token handler");

            self.save_register_factory(&mut storage, register_nft_factory_events)
                .await
                .expect("failed to add register tokens to the database");
        }
    }
}

#[must_use]
pub fn run_register_factory_handler(
    db_pool: ConnectionPool,
    eth_watch_req: mpsc::Sender<EthWatchRequest>,
    config: &ZkSyncConfig,
) -> JoinHandle<()> {
    let config = config.clone();
    tokio::spawn(async move {
        let mut handler =
            NFTFactoryHandler::new(db_pool, eth_watch_req, config.token_handler.clone()).await;

        handler.run().await
    })
}
