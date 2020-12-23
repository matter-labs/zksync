use std::time::Instant;

use futures::{
    channel::mpsc::{self, Receiver, Sender},
    SinkExt, StreamExt,
};

pub struct Balancer<REQUESTS> {
    channels: Vec<Sender<REQUESTS>>,
    requests: Receiver<REQUESTS>,
}

pub trait Balanced<REQUESTS> {
    fn clone_with_receiver(&self, receiver: Receiver<REQUESTS>) -> Self;
}

impl<REQUESTS> Balancer<REQUESTS> {
    pub fn new<T>(
        balanced_item: T,
        requests: Receiver<REQUESTS>,
        number_of_items: u8,
        channel_capacity: usize,
    ) -> (Self, Vec<T>)
    where
        T: Balanced<REQUESTS> + Sync + Send + 'static,
    {
        let mut balanced_items = vec![];
        let mut channels = vec![];

        for _ in 0..number_of_items {
            let (request_sender, request_receiver) = mpsc::channel(channel_capacity);
            channels.push(request_sender);
            balanced_items.push(balanced_item.clone_with_receiver(request_receiver));
        }

        (Self { channels, requests }, balanced_items)
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
    use crate::balancer::{Balanced, Balancer};
    use futures::channel::mpsc;
    use futures::channel::mpsc::Receiver;
    use futures::{SinkExt, StreamExt};

    struct SomeBalancedItem {
        receiver: Receiver<i32>,
    }

    impl Balanced<i32> for SomeBalancedItem {
        fn clone_with_receiver(&self, receiver: Receiver<i32>) -> Self {
            Self { receiver }
        }
    }

    #[tokio::test]
    async fn load_balance() {
        let (mut request_sender, request_receiver) = mpsc::channel(2);
        let (_, tmp_request_receiver) = mpsc::channel(2);

        let (balancer, mut items) = Balancer::new(
            SomeBalancedItem {
                receiver: tmp_request_receiver,
            },
            request_receiver,
            10,
            2,
        );

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
