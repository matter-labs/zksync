//! Token handler is a crate that receives a notification about adding tokens to the contract
//! and adds them to the database.
//!
//! To set the name and the decimals parameter for the token, a match is searched for with the
//! token list (which is taken from the environment). If the token address is not found in the
//! trusted token list, then the default values are used (name = "ERC20-{id}", decimals = 18).

// Built-in deps
use std::collections::HashMap;
// External uses
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
use tokio::task::JoinHandle;
// Workspace uses
use zksync_config::{TokenHandlerConfig, ZkSyncConfig};
use zksync_storage::{tokens::TokensSchema, ConnectionPool, StorageProcessor};
use zksync_types::{
    tokens::{NewTokenEvent, Token, TokenInfo},
    Address,
};
// Local uses
use crate::eth_watch::EthWatchRequest;

struct TokenHandler {
    connection_pool: ConnectionPool,
    poll_interval: std::time::Duration,
    eth_watch_req: mpsc::Sender<EthWatchRequest>,
    token_list: HashMap<Address, TokenInfo>,
    last_token_id: u16,
}

impl TokenHandler {
    async fn new(
        connection_pool: ConnectionPool,
        eth_watch_req: mpsc::Sender<EthWatchRequest>,
        config: TokenHandlerConfig,
    ) -> Self {
        let poll_interval = config.poll_interval();
        let token_list = config
            .token_list
            .into_iter()
            .map(|token| (token.address, token))
            .collect::<HashMap<Address, TokenInfo>>();

        let mut storage = connection_pool
            .access_storage()
            .await
            .expect("db connection failed for token handler");
        let last_token_id = TokensSchema(&mut storage)
            .get_last_token_id()
            .await
            .expect("failed to load last token id");

        drop(storage);

        Self {
            connection_pool,
            eth_watch_req,
            last_token_id,
            token_list,
            poll_interval,
        }
    }

    async fn load_new_token_events(&self) -> Vec<NewTokenEvent> {
        let eth_watch_resp = oneshot::channel();
        self.eth_watch_req
            .clone()
            .send(EthWatchRequest::GetNewTokens {
                token_start_id: self.last_token_id + 1,
                resp: eth_watch_resp.0,
            })
            .await
            .expect("ETH watch req receiver dropped");

        eth_watch_resp.1.await.expect("Err response from eth watch")
    }

    async fn save_new_tokens(
        &self,
        storage: &mut StorageProcessor<'_>,
        tokens: Vec<Token>,
    ) -> anyhow::Result<()> {
        let mut transaction = storage.start_transaction().await?;

        for token in tokens {
            TokensSchema(&mut transaction).store_token(token).await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    async fn run(&mut self) {
        let mut timer = tokio::time::interval(self.poll_interval);
        loop {
            timer.tick().await;

            let new_tokens = self
                .load_new_token_events()
                .await
                .into_iter()
                .map(|token| {
                    // Find a token in the list of trusted tokens
                    // or use default values (name = "ERC20-{id}", decimals = 18).
                    let (symbol, decimals) = {
                        let token_from_list = self.token_list.get(&token.address).cloned();

                        if let Some(token) = token_from_list {
                            (token.symbol, token.decimals)
                        } else {
                            (format!("ERC20-{}", token.id), 18)
                        }
                    };

                    Token::new(token.id, token.address, &symbol, decimals)
                })
                .collect::<Vec<_>>();

            self.last_token_id = new_tokens.iter().map(|token| token.id.0).max().unwrap_or(0);

            let mut storage = self
                .connection_pool
                .access_storage()
                .await
                .expect("db connection failed for token handler");

            self.save_new_tokens(&mut storage, new_tokens)
                .await
                .expect("failed to add tokens to the database");
        }
    }
}

#[must_use]
pub fn run_token_handler(
    db_pool: ConnectionPool,
    eth_watch_req: mpsc::Sender<EthWatchRequest>,
    config: &ZkSyncConfig,
) -> JoinHandle<()> {
    let config = config.clone();
    tokio::spawn(async move {
        let mut token_handler =
            TokenHandler::new(db_pool, eth_watch_req, config.token_handler.clone()).await;

        token_handler.run().await
    })
}
