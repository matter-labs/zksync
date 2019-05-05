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

    fn set(account: u32, nonce: u32) {
        // TODO: notify runtime
    }
}

impl Future for NonceReadyFuture
{
    type Item = ();
    type Error = timer::Error;

    fn poll(&mut self) -> Poll<(), Self::Error> {
        // TODO: if nonce equals, return Async::Ready(())
        Ok(Async::NotReady)
    }
}

fn main() {
    let nm = AsyncNonceMap::default();
    let task = Interval::new(Instant::now(), Duration::from_millis(1000))
    .fold((0, nm), |acc, _| {
        let (i, nm) = acc;
        println!("i = {}", i);
        Ok((i + 1, nm))
    })
    .map_err(|e| panic!("err={:?}", e));
    tokio::run(task.map(|_| ()));
}