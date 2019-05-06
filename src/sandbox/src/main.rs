 #![allow(warnings)]

extern crate iron;
extern crate bodyparser;
extern crate persistent;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate tokio;
extern crate futures;

use futures::{Future, Async, Poll};
use tokio::prelude::*;
use futures::task::{self, Task};
use future::Shared;
use tokio::timer::{self, Interval};
use std::time::{Duration, Instant};
use std::sync::{Arc, RwLock, Mutex};
use std::collections::HashMap;

mod nonce_futures;

use nonce_futures::*;

fn main() {
    let nm = NonceFutures::default();
    let task = Interval::new(Instant::now(), Duration::from_millis(1000))
    .fold((0, nm.clone()), |acc, _| {
        let (i, mut nm) = acc;
        println!("i = {}", i);

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
            let task = nm.await(1, i)
                .timeout(Duration::from_millis(5000))
                .map(|_| println!("success!"))
                .or_else(|e| {println!("error: {:?}", e); future::ok(())} );
            tokio::spawn(task);
        }

        future::ok(())
    }));
}