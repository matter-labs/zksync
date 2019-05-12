use futures::{Future, Async, Poll};
use tokio::prelude::*;
use futures::task::{self, Task};
use future::Shared;
use tokio::timer::{self, Interval};
use std::time::{Duration, Instant};
use std::sync::{Arc, RwLock, Mutex};
use std::collections::HashMap;

pub struct NonceReadyFuture{
    account:            u32,
    nonce:              u32,
    futures:            NonceFutures,
    immediate_result:   Option<Result<(), CurrentNonceIsHigher>>,
}

#[derive(Clone, Debug)]
pub struct CurrentNonceIsHigher;

type NonceFuture = Future<Item=(), Error=CurrentNonceIsHigher>;

#[derive(Default, Clone)]
struct Data{
    nonces:     HashMap<u32, u32>,
    futures:    HashMap<(u32, u32), Shared<NonceReadyFuture>>,
    tasks:      HashMap<(u32, u32), Task>,
}

#[derive(Default, Clone)]
pub struct NonceFutures(Arc<RwLock<Data>>);

impl NonceFutures {
    
    pub fn await(&self, account: u32, nonce: u32) -> impl Future<Item=(), Error=CurrentNonceIsHigher> {

        // get mutex access to inner data
        let data = &mut self.0.as_ref().write().unwrap();

        let record = data.nonces.get(&account).map(|&v|v).clone();
        if record.is_none() {
            // so that we iterate through the notify listing starting not with 0, 
            // but with the first requested nonce
            data.nonces.insert(account, nonce);
        }
        let next_nonce = record.unwrap_or(0);

        // return immediate result if it can be deducted now
        if next_nonce > nonce {
            NonceReadyFuture{
                account,
                nonce,
                futures: self.clone(),
                immediate_result: Some(Err(CurrentNonceIsHigher)),
            }.shared()
        } else if next_nonce == nonce {
                NonceReadyFuture{
                account,
                nonce,
                futures: self.clone(),
                immediate_result: Some(Ok(())),
            }.shared()
        } else {
            // otherwise add future to the list to be notified
            let key = (account, nonce);
            data.futures.get(&key)
            .map(|f| f.clone())
            .unwrap_or_else( || {
                let future = NonceReadyFuture{
                    account,
                    nonce,
                    futures: self.clone(),
                    immediate_result: None,
                }
                .shared();
                let r = future.clone();
                data.futures.insert(key, future);
                r
            })
        }
        .map_err(|_|CurrentNonceIsHigher)
        .map(|_|())
    }

    pub fn set(&mut self, account: u32, new_nonce: u32) {
        // get mutex access to inner data
        let data = &mut self.0.as_ref().write().unwrap();
        
        let old_nonce = *data.nonces.get(&account).unwrap_or(&new_nonce);
        data.nonces.insert(account, new_nonce);

        // notify all tasks which are waiting
        for nonce in old_nonce ..= new_nonce {
            let key = (account, nonce);
            if let Some(task) = data.tasks.remove(&key) {
                task.notify();
                data.futures.remove(&key);
            }
        }
    }
}

impl Future for NonceReadyFuture{
    type Item = ();
    type Error = CurrentNonceIsHigher;

    fn poll(&mut self) -> Poll<(), Self::Error> {

        // return immediate result if present
        if let Some(result) = self.immediate_result.clone() {
            return result.map(|_| Async::Ready(()));
        }

        // get mutex access to inner data
        let data = &mut self.futures.0.as_ref().write().unwrap();

        let next = *data.nonces.get(&self.account).unwrap_or(&0);

        if next > self.nonce {
            Err(CurrentNonceIsHigher)
        } else if next == self.nonce {
            Ok(Async::Ready(()))
        } else {
            // add task to notify when awaited nonce is ready
            let key = (self.account, self.nonce);
            data.tasks.insert(key, task::current());
            Ok(Async::NotReady)
        }
    }
}
