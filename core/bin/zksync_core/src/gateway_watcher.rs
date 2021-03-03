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
    #[error("Hash verification failed: {0} != {1}")]
    InvalidHash(H256, H256),
    #[error("Difference between block numbers is greater than 1: {0} > {1}")]
    InvalidNumDiff(U64, U64),
    #[error("Invalid block: {0:?}")]
    InvalidBlock(Block<H256>),
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
        vlog::info!("Gateway Watcher started");
        time::interval(self.interval)
            .for_each_concurrent(self.task_limit, |_| self.verify_multiplexer_gateways())
            .await
    }

    fn check_blocks(
        latest_block: &Block<H256>,
        block_to_check: &Block<H256>,
    ) -> Result<(), BlockVerificationError> {
        macro_rules! block_opt {
            ($block: expr, $opt: ident) => {
                $block
                    .$opt
                    .ok_or_else(|| BlockVerificationError::InvalidBlock($block.clone()))
            };
        }

        let (lat_phash, lat_hash, lat_num) = (
            latest_block.parent_hash,
            block_opt!(latest_block, hash)?,
            block_opt!(latest_block, number)?,
        );
        let (hash, num) = (
            block_opt!(block_to_check, hash)?,
            block_opt!(block_to_check, number)?,
        );

        if lat_num == num {
            if lat_hash != hash {
                Err(BlockVerificationError::InvalidHash(lat_hash, hash))
            } else {
                Ok(())
            }
        } else if lat_num - num > U64::from(1u64) {
            Err(BlockVerificationError::InvalidNumDiff(lat_num, num))
        } else if lat_phash != hash {
            Err(BlockVerificationError::InvalidHash(lat_phash, hash))
        } else {
            Ok(())
        }
    }

    async fn verify_multiplexer_gateways(&self) {
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
                vlog::error!("Request to Gateway `{}` failed", key),
                retry_delay,
                timeout
            }
            .await
            {
                Some((key, block))
            } else {
                vlog::error!(
                    "Failed to get latest block from Gateway `{}` within specified timeout",
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
        .buffer_unordered(self.req_per_task_limit.unwrap_or_default())
        .filter_map(ready)
        .collect::<Vec<_>>()
        .await;

        if let Some(latest_block) = client_latest_blocks
            .iter()
            .map(|(_, block)| block)
            .max_by(|block1, block2| block1.number.cmp(&block2.number))
        {
            for (key, block) in &client_latest_blocks {
                if let Err(err) = Self::check_blocks(latest_block, block) {
                    vlog::error!("Gateway `{}` - check failed: {:?}", key, err);
                }
            }
        }
    }

    pub fn from_config(config: &ZkSyncConfig) -> Self {
        Self::new(
            EthereumGateway::from_config(config),
            config.gateway_watcher.request_per_task_limit(),
            config.gateway_watcher.task_limit(),
            config.gateway_watcher.check_interval(),
            config.gateway_watcher.request_timeout(),
            config.gateway_watcher.retry_delay(),
        )
    }
}

#[must_use]
pub fn run_gateway_watcher(config: &ZkSyncConfig) -> JoinHandle<()> {
    tokio::spawn(GatewayWatcher::from_config(config).run())
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
        assert_eq!(GatewayWatcher::check_blocks(&b1, &b2), Ok(()));
        b2.hash = Some(h2);
        assert_eq!(
            GatewayWatcher::check_blocks(&b1, &b2),
            Err(BlockVerificationError::InvalidHash(h1, h2))
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
        assert_eq!(GatewayWatcher::check_blocks(&b1, &b2), Ok(()));
        b2.hash = Some(h1);
        assert_eq!(
            GatewayWatcher::check_blocks(&b1, &b2),
            Err(BlockVerificationError::InvalidHash(h2, h1))
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
            GatewayWatcher::check_blocks(&b1, &b2),
            Err(BlockVerificationError::InvalidNumDiff(
                U64::from(2u64),
                U64::from(0u64)
            ))
        );
    }
}
