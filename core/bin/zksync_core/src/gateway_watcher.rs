use super::retry_opt_fut;
use futures::{future::ready, stream, StreamExt};
use std::iter;
use std::time::Duration;
use thiserror::Error;
use tokio::{task::JoinHandle, time};

use zksync_config::ZkSyncConfig;
use zksync_eth_client::EthereumGateway;

use web3::types::{Block, BlockId, BlockNumber, H256, U64};
use zksync_eth_client::ETHDirectClient;
use zksync_eth_signer::PrivateKeySigner;

pub struct GatewayWatcher<T> {
    client: T,
    req_per_task_limit: Option<usize>,
    task_limit: Option<usize>,
    interval: Duration,
    req_timeout: Duration,
    retry_delay: Duration,
}

#[derive(Error, Debug, PartialEq)]
enum BlockVerificationError {
    #[error("Hash verification failed: {0:?} != {1:?}")]
    IncorrectHash(H256, H256),
    #[error("Difference between block numbers is greater than 1: {0:?} > {1:?}")]
    LargeNumDiff(U64, U64),
    #[error("Invalid block: {0:?}")]
    InvalidBlock(Box<Block<H256>>),
}

impl GatewayWatcher<EthereumGateway> {
    pub fn new(
        client: EthereumGateway,
        req_per_task_limit: impl Into<Option<usize>>,
        task_limit: impl Into<Option<usize>>,
        interval: Duration,
        req_timeout: Duration,
        retry_delay: Duration,
    ) -> Self {
        Self {
            client,
            req_per_task_limit: req_per_task_limit.into(),
            task_limit: task_limit.into(),
            interval,
            retry_delay,
            req_timeout,
        }
    }

    pub async fn run(self) {
        vlog::info!("Ethereum Gateway Watcher started");

        time::interval(self.interval)
            .for_each_concurrent(self.task_limit, |_| self.check_multiplexer_gateways())
            .await
    }

    fn verify_blocks(
        latest_block: &Block<H256>,
        block_to_check: &Block<H256>,
    ) -> Result<(), BlockVerificationError> {
        macro_rules! block_opt {
            ($block: expr, $opt: ident) => {
                $block
                    .$opt
                    .ok_or_else(|| BlockVerificationError::InvalidBlock(Box::new($block.clone())))?
            };
        }

        let (lat_phash, lat_hash, lat_num) = (
            latest_block.parent_hash,
            block_opt!(latest_block, hash),
            block_opt!(latest_block, number),
        );
        let (hash, num) = (
            block_opt!(block_to_check, hash),
            block_opt!(block_to_check, number),
        );

        if lat_num == num {
            if lat_hash != hash {
                Err(BlockVerificationError::IncorrectHash(lat_hash, hash))
            } else {
                Ok(())
            }
        } else if lat_num - num > U64::from(1u64) {
            Err(BlockVerificationError::LargeNumDiff(lat_num, num))
        } else if lat_phash != hash {
            Err(BlockVerificationError::IncorrectHash(lat_phash, hash))
        } else {
            Ok(())
        }
    }

    async fn check_multiplexer_gateways(&self) {
        let client = match self.client {
            EthereumGateway::Multiplexed(ref client) => client,
            _ => {
                return;
            }
        };

        async fn get_latest_client_block<'a>(
            ((key, client), (retry_delay, timeout)): (
                (&'a str, &'a ETHDirectClient<PrivateKeySigner>),
                (Duration, Duration),
            ),
        ) -> Option<(&'a str, Block<H256>)> {
            if let Ok(block) = retry_opt_fut! {
                async {
                    client
                        .block(BlockId::from(BlockNumber::Latest))
                        .await
                        .ok()
                        .flatten()
                },
                vlog::error!("Request to Ethereum Gateway `{}` failed", key),
                retry_delay,
                timeout
            }
            .await
            {
                Some((key, block))
            } else {
                vlog::error!(
                    "Failed to get latest block from Ethereum Gateway `{}` within specified timeout",
                    key
                );
                None
            }
        }

        let client_latest_blocks = stream::iter(
            client
                .clients()
                .zip(iter::repeat((self.retry_delay, self.req_timeout))),
        )
        .map(get_latest_client_block)
        .buffer_unordered(self.req_per_task_limit.unwrap_or(usize::MAX))
        .filter_map(ready)
        .collect::<Vec<_>>()
        .await;

        if let Some((preferred_key, latest_block)) = client_latest_blocks
            .iter()
            .max_by(|(_, block1), (_, block2)| block1.number.cmp(&block2.number))
        {
            client.prioritize_client(preferred_key);
            for (key, block) in &client_latest_blocks {
                if let Err(err) = Self::verify_blocks(latest_block, block) {
                    vlog::error!("Ethereum Gateway `{}` - check failed: {}", key, err);
                }
            }
        }
    }
}

#[must_use]
pub fn run_gateway_watcher(eth_gateway: EthereumGateway, config: &ZkSyncConfig) -> JoinHandle<()> {
    let gateway_watcher = GatewayWatcher::new(
        eth_gateway,
        config.gateway_watcher.request_per_task_limit(),
        config.gateway_watcher.task_limit(),
        config.gateway_watcher.check_interval(),
        config.gateway_watcher.request_timeout(),
        config.gateway_watcher.retry_delay(),
    );

    tokio::spawn(gateway_watcher.run())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_same_depth_block_hash_check() {
        let h1 = H256::random();
        let h2 = H256::random();

        let mut b1 = Block::default();
        let mut b2 = Block::default();
        b1.hash = Some(h1);
        b2.hash = Some(h1);
        b1.number = Some(U64::from(1u64));
        b2.number = Some(U64::from(1u64));
        assert_eq!(GatewayWatcher::verify_blocks(&b1, &b2), Ok(()));
        b2.hash = Some(h2);
        assert_eq!(
            GatewayWatcher::verify_blocks(&b1, &b2),
            Err(BlockVerificationError::IncorrectHash(h1, h2))
        );
    }

    #[test]
    fn test_different_depth_block_hash_check() {
        let h1 = H256::random();
        let h2 = H256::random();

        let mut b1 = Block::default();
        let mut b2 = Block::default();
        b1.hash = Some(h1);
        b1.parent_hash = h2;
        b2.hash = Some(h2);

        b1.number = Some(U64::from(1u64));
        b2.number = Some(U64::from(0u64));
        assert_eq!(GatewayWatcher::verify_blocks(&b1, &b2), Ok(()));
        b2.hash = Some(h1);
        assert_eq!(
            GatewayWatcher::verify_blocks(&b1, &b2),
            Err(BlockVerificationError::IncorrectHash(h2, h1))
        );
    }

    #[test]
    fn test_block_incorrect_depth_check() {
        let h1 = H256::random();
        let h2 = H256::random();

        let mut b1 = Block::default();
        let mut b2 = Block::default();
        b1.hash = Some(h1);
        b2.hash = Some(h2);

        b1.number = Some(U64::from(2u64));
        b2.number = Some(U64::from(0u64));
        assert_eq!(
            GatewayWatcher::verify_blocks(&b1, &b2),
            Err(BlockVerificationError::LargeNumDiff(
                U64::from(2u64),
                U64::from(0u64)
            ))
        );
    }
}
