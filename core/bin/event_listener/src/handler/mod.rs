use futures::channel::mpsc;
use futures::StreamExt;
use std::collections::{HashMap, HashSet};
use subscribers::Subscriber;
use zksync_storage::event::{records::EventType, types::ZkSyncEvent};

use filters::*;
use zksync_storage::event::types::block::*;

mod filters;
mod subscribers;

pub struct EventHandler {
    rx_for_events: mpsc::Receiver<Vec<ZkSyncEvent>>,
    subs: HashSet<Subscriber>,
    // TODO: sender/reciever to communicate with transport component.
}

impl EventHandler {
    pub fn new(receiver: mpsc::Receiver<Vec<ZkSyncEvent>>) -> Self {
        Self {
            rx_for_events: receiver,
            subs: HashSet::new(),
        }
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        /* Testing filters */
        let block_filter = EventFilter::Block(BlockFilter {
            block_status: Some(BlockStatus::Finalized),
        });
        let account_filter = EventFilter::Account(AccountFilter {
            account_ids: Some([1, 10].iter().cloned().collect::<HashSet<i64>>()),
            token_ids: None,
            status: None,
        });

        self.subs.insert(Subscriber {
            id: 0,
            filters: {
                let mut filters = HashMap::new();
                filters.insert(EventType::Block, block_filter);
                filters
            },
        });

        self.subs.insert(Subscriber {
            id: 1,
            filters: {
                let mut filters = HashMap::new();
                filters.insert(EventType::Account, account_filter);
                filters
            },
        });
        /* Testing filters */

        while let Some(events) = self.rx_for_events.next().await {
            for event in &events {
                for sub in self.subs.iter() {
                    if sub.matches(event) {
                        eprintln!("Sub id: {}\nEvent: {:?}", sub.id, event);
                    }
                }
            }
        }

        Ok(())
    }
}

#[must_use]
pub fn run_event_handler(
    receiver: mpsc::Receiver<Vec<ZkSyncEvent>>,
) -> tokio::task::JoinHandle<()> {
    let mut handler = EventHandler::new(receiver);
    tokio::spawn(async move {
        handler.run().await.unwrap();
    })
}
