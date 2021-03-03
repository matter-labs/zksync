use std::time::Instant;

use futures::{
    channel::mpsc::{self, Receiver, Sender},
    SinkExt, StreamExt,
};

pub struct Balancer<R> {
    channels: Vec<Sender<R>>,
    requests: Receiver<R>,
}

pub trait BuildBalancedItem<R, S> {
    fn build_with_receiver(&self, receiver: Receiver<R>) -> S;
}

impl<R> Balancer<R> {
    pub fn new<T, S>(
        balanced_item: T,
        requests: Receiver<R>,
        number_of_items: u8,
        channel_capacity: usize,
    ) -> (Self, Vec<S>)
    where
        T: BuildBalancedItem<R, S> + Sync + Send + 'static,
    {
        let mut balanced_items = vec![];
        let mut channels = vec![];

        for _ in 0..number_of_items {
            let (request_sender, request_receiver) = mpsc::channel(channel_capacity);
            channels.push(request_sender);
            balanced_items.push(balanced_item.build_with_receiver(request_receiver));
        }

        (Self { channels, requests }, balanced_items)
    }

    pub async fn run(mut self) {
        // It's an obvious way of balancing. Send an equal number of requests to each ticker
        let mut channel_indexes = (0..self.channels.len()).into_iter().cycle();
        // It's the easiest way how to cycle over channels, because cycle required clone trait.
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
    use crate::{Balancer, BuildBalancedItem};
    use futures::channel::mpsc;
    use futures::channel::mpsc::Receiver;
    use futures::{SinkExt, StreamExt};

    struct SomeBalancedItemBuilder;
    struct SomeBalancedItem {
        receiver: Receiver<i32>,
    }

    impl BuildBalancedItem<i32, SomeBalancedItem> for SomeBalancedItemBuilder {
        fn build_with_receiver(&self, receiver: Receiver<i32>) -> SomeBalancedItem {
            SomeBalancedItem { receiver }
        }
    }

    #[tokio::test]
    async fn load_balance() {
        let (mut request_sender, request_receiver) = mpsc::channel(2);

        let (balancer, mut items) = Balancer::new(SomeBalancedItemBuilder, request_receiver, 10, 2);

        tokio::spawn(balancer.run());
        for i in 0..50 {
            request_sender.send(i).await.unwrap();
            if let Some(res) = items[(i % 10) as usize].receiver.next().await {
                assert_eq!(res, i)
            } else {
                panic!("Wrong type")
            }
        }
    }
}
