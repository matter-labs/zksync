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

#[derive(Default, Clone)]
struct AsyncNonceMap{
    nonces:     Arc<RwLock<HashMap<u32, u32>>>,
    futures:    Arc<RwLock<HashMap<(u32, u32), Shared<NonceReadyFuture>>>>,
    tasks:      Arc<RwLock<HashMap<(u32, u32), Task>>>,
}

impl AsyncNonceMap {
    fn await(&self, account: u32, nonce: u32) -> Shared<NonceReadyFuture> {
        let futures = &mut self.futures.as_ref().write().unwrap();
        let key = (account, nonce);
        futures.get(&key)
        .map(|f| f.clone())
        .unwrap_or_else( || {
            let future = NonceReadyFuture{
                account,
                nonce,
                nonce_map: self.clone(),
            }.shared();
            let r = future.clone();
            futures.insert(key, future);
            r
        })
    }

    fn set(&mut self, account: u32, nonce: u32) {
        let mut map = &mut self.nonces.as_ref().write().unwrap();
        map.insert(account, nonce);
        let tasks = &mut self.tasks.as_ref().write().unwrap();
        let key = (account, nonce);
        if let Some(task) = tasks.remove(&key) {
            task.notify();
        }
    }
}

struct NonceReadyFuture{
    account:    u32,
    nonce:      u32,
    nonce_map:  AsyncNonceMap,
}

impl Future for NonceReadyFuture
{
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<(), Self::Error> {
        let map = &self.nonce_map.nonces.as_ref().read().unwrap();
        let next = *map.get(&self.account).unwrap_or(&0);
        if next == self.nonce {
            Ok(Async::Ready(()))
        } else {
            let tasks = &mut self.nonce_map.tasks.as_ref().write().unwrap();
            let key = (self.account, self.nonce);
            tasks.insert(key, task::current());
            Ok(Async::NotReady)
        }
    }
}

fn main() {
    let nm = AsyncNonceMap::default();
    let task = Interval::new(Instant::now(), Duration::from_millis(1000))
    .fold((0, nm.clone()), |acc, _| {
        let (i, mut nm) = acc;
        println!("i = {}", i);

        if i == 3 {
            nm.set(1, 2);
        }

        let next = (i + 1, nm);
        future::ok(next)
    })
    .map_err(|e| panic!("err={:?}", e));

    tokio::run(future::lazy(move || {
        tokio::spawn(task.map(|_| ()));

        for i in 0..2 {
            let task = nm.await(1, 2)
                .timeout(Duration::from_millis(5000))
                .map(|_| println!("success!"))
                .or_else(|_| {println!("timout"); future::ok(())} );
            tokio::spawn(task);
        }

        future::ok(())
    }));
}