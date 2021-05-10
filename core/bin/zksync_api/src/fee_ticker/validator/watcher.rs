use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use bigdecimal::{BigDecimal, Zero};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use zksync_types::{Address, Token};

use crate::fee_ticker::ticker_api::REQUEST_TIMEOUT;

#[async_trait::async_trait]
pub trait TokenWatcher {
    async fn get_token_market_volume(&mut self, token: &Token) -> anyhow::Result<BigDecimal>;
}

/// Watcher for Uniswap protocol
/// https://thegraph.com/explorer/subgraph/uniswap/uniswap-v2
#[derive(Clone)]
pub struct UniswapTokenWatcher {
    client: reqwest::Client,
    addr: String,
    cache: Arc<Mutex<HashMap<Address, BigDecimal>>>,
}

impl UniswapTokenWatcher {
    pub fn new(addr: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            addr,
            cache: Default::default(),
        }
    }
    async fn get_market_volume(&mut self, address: Address) -> anyhow::Result<BigDecimal> {
        // Uniswap has graphql API, using full graphql client for one query is overkill for current task
        // TODO https://linear.app/matterlabs/issue/ZKS-413/support-full-version-of-graphql-for-tokenvalidator
        let start = Instant::now();

        let query = format!("{{token(id: \"{:#x}\"){{untrackedVolumeUSD}}}}", address);

        let raw_response = self
            .client
            .post(&self.addr)
            .json(&serde_json::json!({
                "query": query.clone(),
            }))
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|err| anyhow::format_err!("Uniswap API request failed: {}", err))?;

        let response_status = raw_response.status();
        let response_text = raw_response.text().await?;

        let response: GraphqlResponse = serde_json::from_str(&response_text).map_err(|err| {
            anyhow::format_err!(
                "Error: {} while decoding response: {} with status: {}",
                err,
                response_text,
                response_status
            )
        })?;

        metrics::histogram!("ticker.uniswap_watcher.get_market_volume", start.elapsed());

        let volume = if let Some(token) = response.data.token {
            token.untracked_volume_usd.parse()?
        } else {
            BigDecimal::zero()
        };
        Ok(volume)
    }
    async fn update_historical_amount(&mut self, address: Address, amount: BigDecimal) {
        let mut cache = self.cache.lock().await;
        cache.insert(address, amount);
    }
    async fn get_historical_amount(&mut self, address: Address) -> Option<BigDecimal> {
        let cache = self.cache.lock().await;
        cache.get(&address).cloned()
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GraphqlResponse {
    pub data: GraphqlTokenResponse,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GraphqlTokenResponse {
    pub token: Option<TokenResponse>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TokenResponse {
    /// Total amount swapped all time in token pair stored in USD, no minimum liquidity threshold.
    #[serde(rename = "untrackedVolumeUSD")]
    pub untracked_volume_usd: String,
}

#[async_trait::async_trait]
impl TokenWatcher for UniswapTokenWatcher {
    async fn get_token_market_volume(&mut self, token: &Token) -> anyhow::Result<BigDecimal> {
        match self.get_market_volume(token.address).await {
            Ok(amount) => {
                self.update_historical_amount(token.address, amount.clone())
                    .await;
                return Ok(amount);
            }
            Err(err) => {
                vlog::error!("Error in api: {:?}", err);
            }
        }

        if let Some(amount) = self.get_historical_amount(token.address).await {
            return Ok(amount);
        };
        anyhow::bail!("Token amount api is not available right now.")
    }
}
