#[macro_use]
extern crate log;

use futures::Future;
use std::time::{Duration, Instant};
use tokio::prelude::*;
use tokio::timer::Interval;

mod nonce_futures;

use nonce_futures::*;

fn main() {
    env_logger::init();

    let nm = NonceFutures::default();
    let task = Interval::new(Instant::now(), Duration::from_millis(1000))
        .fold((0, nm.clone()), |acc, _| {
            let (i, mut nm) = acc;
            info!("i = {}", i);

            if i == 2 {
                nm.set(1, 2);
            }

            let next = (i + 1, nm);
            future::ok(next)
        })
        .map_err(|e| panic!("err={:?}", e));

    tokio::run(future::lazy(move || {
        tokio::spawn(task.map(|_| ()));

        for i in 0..=4 {
            let task = nm
                .nonce_await(1, i)
                .timeout(Duration::from_millis(5000))
                .map(|_| info!("success!"))
                .or_else(|e| {
                    error!("error: {:?}", e);
                    future::ok(())
                });
            tokio::spawn(task);
        }

        future::ok(())
    }));
}
