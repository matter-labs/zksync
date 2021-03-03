use std::time::Duration;

use futures::{future::ready, stream, StreamExt};
use tokio::time;
use web3::types::{Block, BlockId, BlockNumber, H256, U64};
use zksync_eth_client::{ETHDirectClient, MultiplexerEthereumClient};
use zksync_eth_signer::PrivateKeySigner;

use zksync_config::ZkSyncConfig;
use zksync_eth_client::EthereumGateway;

macro_rules! retry_fut {
    ($fut: expr, $err_expr: expr, $delay_ms: expr) => {
        async {
            loop {
                if let Ok(val) = $fut.await {
                    break val;
                } else {
                    $err_expr;
                    time::delay_for(Duration::from_millis($delay_ms)).await;
                }
            }
        }
    };
}

const RETRY_DELAY: u64 = 1000;

async fn eth_web3_gateway_watcher(client: &MultiplexerEthereumClient) {
    async fn get_latest_block<'a>(
        (key, client): (&'a str, &ETHDirectClient<PrivateKeySigner>),
    ) -> Option<(&'a str, Block<H256>)> {
        if let Some(latest_block) = retry_fut!(
            client.block(BlockId::from(BlockNumber::Latest)),
            vlog::error!("Request to node {} failed", key),
            RETRY_DELAY
        )
        .await
        {
            Some((key, latest_block))
        } else {
            vlog::error!("Node {} responded with empty latest block", key);
            None
        }
    }

    let clients_latest_block = stream::iter(client.clients())
        .map(get_latest_block)
        .buffer_unordered(10)
        .filter_map(ready)
        .collect::<Vec<_>>()
        .await;

    if let Some((latest_parent_hash, latest_hash, latest_num)) = clients_latest_block
        .iter()
        .map(|(_, block)| block)
        .max_by(|block1, block2| block1.number.cmp(&block2.number))
        .map(|last_block| {
            (
                last_block.parent_hash,
                last_block.hash.expect("Invalid block"),
                last_block.number.expect("Invalid block"),
            )
        })
    {
        for (key, block) in &clients_latest_block {
            let (hash, num) = (
                block.hash.expect("Invalid block"),
                block.number.expect("Invalid block"),
            );

            if latest_num == num {
                if latest_hash != hash {
                    vlog::error!(
                        "Block id check failed for {}: {:?} != {:?}",
                        key,
                        latest_hash,
                        hash
                    );
                }
            } else if latest_num - num > U64::from(1u64) {
                vlog::error!(
                    "Difference between block numbers is greater than 1 for {}: {:?} > {:?}",
                    key,
                    latest_num,
                    num,
                );
            } else if latest_parent_hash != hash {
                vlog::error!(
                    "Latest block parent hash verification failed for {}: {:?} != {:?}",
                    key,
                    latest_parent_hash,
                    hash
                );
            }
        }
    }
}

#[tokio::main]
async fn main() {
    vlog::init();

    let config = ZkSyncConfig::from_env();
    let client = EthereumGateway::from_config(&config);

    match client {
        EthereumGateway::Multiplexed(client) => {
            vlog::info!("ETH web3 gateway watcher started");

            time::interval(Duration::from_millis(
                config.eth_watch.eth_node_poll_interval,
            ))
            .for_each_concurrent(None, |_| eth_web3_gateway_watcher(&client))
            .await
        }
        _ => {
            vlog::info!("ETH web3 gateway watcher: connection is not `Multiplexed`, shutting down");
        }
    }
}
