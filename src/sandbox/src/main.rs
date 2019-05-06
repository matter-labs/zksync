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
struct Data{
    nonces:     HashMap<u32, u32>,
    futures:    HashMap<(u32, u32), Shared<NonceReadyFuture>>,
    tasks:      HashMap<(u32, u32), Task>,
}

#[derive(Default, Clone)]
struct NonceFutures(Arc<RwLock<Data>>);

impl NonceFutures {
    fn await(&self, account: u32, nonce: u32) -> Shared<NonceReadyFuture> {
        let data = &mut self.0.as_ref().write().unwrap();
        let key = (account, nonce);

        // TODO: check status, return properly

        data.futures.get(&key)
        .map(|f| f.clone())
        .unwrap_or_else( || {
            let future = NonceReadyFuture{
                account,
                nonce,
                futures: self.clone(),
            }.shared();
            let r = future.clone();
            data.futures.insert(key, future);
            r
        })
    }

    fn set(&mut self, account: u32, new_nonce: u32) {
        let data = &mut self.0.as_ref().write().unwrap();
        let old_nonce = *data.nonces.get(&account).unwrap_or(&0);
        data.nonces.insert(account, new_nonce);

        for nonce in old_nonce ..= new_nonce {
            let key = (account, nonce);
            if let Some(task) = data.tasks.remove(&key) {
                task.notify();
                data.futures.remove(&key);
            }
        }
    }
}

struct NonceReadyFuture{
    account:    u32,
    nonce:      u32,
    futures:    NonceFutures,
}

struct CurrentNonceIsHigher;

impl Future for NonceReadyFuture{
    type Item = ();
    type Error = CurrentNonceIsHigher;

    fn poll(&mut self) -> Poll<(), Self::Error> {
        let data = &mut self.futures.0.as_ref().write().unwrap();
        let key = (self.account, self.nonce);
        let next = *data.nonces.get(&self.account).unwrap_or(&0);
        if next > self.nonce {
            Err(CurrentNonceIsHigher)
        } else if next == self.nonce {
            Ok(Async::Ready(()))
        } else {
            data.tasks.insert(key, task::current());
            Ok(Async::NotReady)
        }
    }
}

fn main() {
    let nm = NonceFutures::default();
    let task = Interval::new(Instant::now(), Duration::from_millis(1000))
    .fold((0, nm.clone()), |acc, _| {
        let (i, mut nm) = acc;
        println!("i = {}", i);

        if i == 1 {
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