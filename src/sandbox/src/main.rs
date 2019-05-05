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
use tokio::timer::{self, Interval};
use std::time::{Duration, Instant};
use std::sync::{Arc, RwLock};
use std::collections::HashMap;

#[derive(Default, Clone)]
struct AsyncNonceMap{
    nonces:     Arc<RwLock<HashMap<u32, u32>>>
}

struct NonceReadyFuture{
    account:    u32,
    nonce:      u32,
    nonce_map:  AsyncNonceMap,
}

impl AsyncNonceMap {
    fn await(&self, account: u32, nonce: u32) -> NonceReadyFuture {
        NonceReadyFuture{
            account,
            nonce,
            nonce_map: self.clone(),
        }
    }

    fn set(&mut self, account: u32, nonce: u32) {
        let mut map = &mut self.nonces.as_ref().write().unwrap();
        map.insert(account, nonce);
        // TODO: notify runtime
    }
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

        let task = nm.await(1, 2)
            .timeout(Duration::from_millis(5000))
            .map(|_| println!("success!"))
            .or_else(|_| {println!("timout"); future::ok(())} );

        tokio::spawn(task);

        future::ok(())
    }));
}