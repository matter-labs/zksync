use futures::{
    channel::mpsc::{self, Receiver, Sender},
    SinkExt, StreamExt,
};
use std::{collections::HashMap, sync::Arc, time::Instant};

use tokio::sync::Mutex;
use zksync_storage::ConnectionPool;

use crate::{
    fee_ticker::{
        ticker_api::{TickerApi, TokenPriceAPI},
        ticker_info::FeeTickerInfo,
        validator::{watcher::TokenWatcher, FeeTokenValidator},
        FeeTicker, TickerConfig, TickerRequest,
    },
    utils::token_db_cache::TokenDBCache,
};

static TICKER_CHANNEL_SIZE: usize = 32000;

/// `TickerBalancer` is a struct used for scaling the ticker.
/// Create `n` tickers and balance the load between them.
pub(crate) struct TickerBalancer<API: TokenPriceAPI, INFO, WATCHER> {
    tickers: Vec<FeeTicker<TickerApi<API>, INFO, WATCHER>>,
    channels: Vec<Sender<TickerRequest>>,
    requests: Receiver<TickerRequest>,
}

impl<API, INFO, WATCHER> TickerBalancer<API, INFO, WATCHER>
where
    API: TokenPriceAPI + Clone + Sync + Send + 'static,
    INFO: FeeTickerInfo + Clone + Sync + Send + 'static,
    WATCHER: TokenWatcher + Clone + Sync + Send + 'static,
{
    pub fn new(
        token_price_api: API,
        ticker_info: INFO,
        ticker_config: TickerConfig,
        validator: FeeTokenValidator<WATCHER>,
        requests: Receiver<TickerRequest>,
        db_pool: ConnectionPool,
        number_of_tickers: u8,
    ) -> Self {
        let mut tickers = vec![];
        let mut channels = vec![];

        let token_db_cache = TokenDBCache::new();
        let price_cache = Arc::new(Mutex::new(HashMap::new()));
        let gas_price_cache = Arc::new(Mutex::new(None));

        for _ in 0..number_of_tickers {
            let ticker_api = TickerApi::new(db_pool.clone(), token_price_api.clone())
                .with_token_db_cache(token_db_cache.clone())
                .with_price_cache(price_cache.clone())
                .with_gas_price_cache(gas_price_cache.clone());
            let (request_sender, request_receiver) = mpsc::channel(TICKER_CHANNEL_SIZE);
            tickers.push(FeeTicker::new(
                ticker_api,
                ticker_info.clone(),
                request_receiver,
                ticker_config.clone(),
                validator.clone(),
            ));
            channels.push(request_sender);
        }

        Self {
            tickers,
            channels,
            requests,
        }
    }
    pub fn spawn_tickers(&mut self) {
        while let Some(ticker) = self.tickers.pop() {
            tokio::spawn(ticker.run());
        }
    }

    pub async fn run(mut self) {
        // It's an obvious way of balancing. Send an equal number of requests to each ticker
        let mut channel_indexes = (0..self.channels.len()).into_iter().cycle();
        // it's the easiest way how to cycle over channels, because cycle required clone trait
        while let Some(request) = self.requests.next().await {
            let channel_index = channel_indexes
                .next()
                .expect("Exactly one channel should exists");
            let start = Instant::now();
            self.channels[channel_index]
                .send(request)
                .await
                .unwrap_or_default();
            metrics::histogram!("ticker.dispatcher.request", start.elapsed());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TickerBalancer;
    use crate::fee_ticker::ticker_api::coingecko::CoinGeckoAPI;
    use crate::fee_ticker::ticker_info::TickerInfo;
    use crate::fee_ticker::validator::watcher::UniswapTokenWatcher;
    use crate::fee_ticker::TickerRequest;
    use futures::{
        channel::{mpsc, oneshot},
        SinkExt, StreamExt,
    };
    use zksync_types::{TokenId, TxFeeTypes};

    #[tokio::test]
    async fn dispatch() {
        let mut receivers = vec![];
        let mut senders = vec![];
        for _ in 0..10 {
            let channel = mpsc::channel(2);
            senders.push(channel.0);
            receivers.push(channel.1);
        }
        let (mut request_sender, request_receiver) = mpsc::channel(2);

        let dispatcher = TickerBalancer::<CoinGeckoAPI, TickerInfo, UniswapTokenWatcher> {
            tickers: vec![],
            channels: senders,
            requests: request_receiver,
        };
        tokio::spawn(dispatcher.run());
        for i in 0..50 {
            let channel = oneshot::channel();
            request_sender
                .send(TickerRequest::GetTxFee {
                    tx_type: TxFeeTypes::Withdraw,
                    token: TokenId(i).into(),
                    address: Default::default(),
                    response: channel.0,
                })
                .await
                .unwrap();
            if let Some(TickerRequest::GetTxFee {
                tx_type: _,
                token,
                address: _,
                response: _,
            }) = receivers[(i % 10) as usize].next().await
            {
                assert_eq!(token, TokenId(i).into());
            } else {
                panic!("Wrong type")
            }
        }
    }
}
