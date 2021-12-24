use std::{collections::VecDeque, time::Duration};

const N_SAMPLES: usize = 10;

#[derive(Debug, Default)]
pub(crate) struct BlockThrottler {
    block_create_time: VecDeque<Duration>,
    block_create_time_sum: Duration,

    root_hash_time: VecDeque<Duration>,
    root_hash_time_sum: Duration,
}

impl BlockThrottler {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn add_block_create_time(&mut self, new_sample: Duration) {
        Self::add_sample(
            &mut self.block_create_time,
            &mut self.block_create_time_sum,
            new_sample,
        );
    }

    pub(crate) fn add_root_hash_time(&mut self, new_sample: Duration) {
        Self::add_sample(
            &mut self.root_hash_time,
            &mut self.root_hash_time_sum,
            new_sample,
        );
    }

    pub(crate) fn throttle_interval(&self) -> Duration {
        todo!()
    }

    pub(crate) async fn throttle(&self) {
        let interval = self.throttle_interval();
        if interval != Duration::ZERO {
            tokio::time::sleep(interval).await;
        }
    }

    fn add_sample(collection: &mut VecDeque<Duration>, sum: &mut Duration, new_sample: Duration) {
        if collection.len() < N_SAMPLES {
            collection.push_back(new_sample);
            *sum += new_sample;
        } else {
            let oldest_sample = collection
                .pop_front()
                .expect("Collection must not be empty");
            *sum -= oldest_sample;
            collection.push_back(new_sample);
            *sum += new_sample;
        }
    }

    fn average(collection: &VecDeque<Duration>, sum: &Duration) -> Duration {
        if collection.is_empty() {
            return Duration::ZERO;
        }

        *sum / (collection.len() as u32)
    }
}
