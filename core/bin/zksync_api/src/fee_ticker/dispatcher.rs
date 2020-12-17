use futures::{
    channel::mpsc::{self, Receiver, Sender},
    SinkExt, StreamExt,
};
use std::time::Instant;
use zksync_storage::ConnectionPool;

use crate::fee_ticker::{
    fee_token_validator::FeeTokenValidator,
    ticker_api::{TickerApi, TokenPriceAPI},
    ticker_info::FeeTickerInfo,
    FeeTicker, TickerConfig, TickerRequest,
};

pub(crate) struct Dispatcher<API: TokenPriceAPI, INFO> {
    tickers: Vec<FeeTicker<TickerApi<API>, INFO>>,
    channels: Vec<Sender<TickerRequest>>,
    requests: Receiver<TickerRequest>,
}

impl<API, INFO> Dispatcher<API, INFO>
where
    API: TokenPriceAPI + Clone + Sync + Send + 'static,
    INFO: FeeTickerInfo + Clone + Sync + Send + 'static,
{
    pub fn new(
        token_price_api: API,
        ticker_info: INFO,
        ticker_config: TickerConfig,
        validator: FeeTokenValidator,
        requests: Receiver<TickerRequest>,
        db_pool: ConnectionPool,
        number_of_tickers: u8,
    ) -> Self {
        let mut tickers = vec![];
        let mut channels = vec![];
        for _ in 0..number_of_tickers {
            let ticker_api = TickerApi::new(db_pool.clone(), token_price_api.clone());
            let (request_sender, request_receiver) = mpsc::channel(3000);
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
    use super::Dispatcher;
    use crate::fee_ticker::ticker_api::coingecko::CoinGeckoAPI;
    use crate::fee_ticker::ticker_info::TickerInfo;
    use crate::fee_ticker::TickerRequest;
    use futures::{
        channel::{mpsc, oneshot},
        SinkExt, StreamExt,
    };
    use zksync_types::TxFeeTypes;

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

        let dispatcher = Dispatcher::<CoinGeckoAPI, TickerInfo> {
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
                    address: Default::default(),
                    token: i.into(),
                    response: channel.0,
                })
                .await
                .unwrap();
            if let Some(TickerRequest::GetTxFee {
                tx_type: _,
                address: _,
                token,
                response: _,
            }) = receivers[(i % 10) as usize].next().await
            {
                assert_eq!(token, i.into());
            } else {
                panic!("Wrong type")
            }
        }
    }
}
