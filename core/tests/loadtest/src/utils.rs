//! Common functions shared by different scenarios.

// Built-in uses
use std::iter::Iterator;
// External uses
use futures::{Future, TryFuture};
use num::BigUint;

use crate::session::save_error;
// Workspace uses
// Local uses

/// Default chunk sizes for the `wait_all_chunks` methods.
pub const CHUNK_SIZES: &[usize] = &[25, 50, 100, 50];

const ERRORS_CUTOFF: usize = 10;

/// Converts "gwei" amount to the "wei".
pub fn gwei_to_wei(gwei: impl Into<BigUint>) -> BigUint {
    gwei.into() * BigUint::from(10u64.pow(9))
}

/// Creates a future which represents a collection of the outputs of the futures
/// given.
///
/// But unlike the `futures::future::join_all` method, it performs futures in chunks
/// to reduce descriptors usage.
pub async fn wait_all_chunks<I>(chunk_sizes: &[usize], i: I) -> Vec<<I::Item as Future>::Output>
where
    I: IntoIterator,
    I::Item: Future,
{
    let mut output = Vec::new();
    for chunk in DynamicChunks::new(i, chunk_sizes) {
        let values = futures::future::join_all(chunk).await;
        output.extend(values);
    }
    output
}

/// Creates a future which represents either a collection of the results of the
/// futures given or an error.
///
/// But unlike the `futures::future::try_join_all` method, it returns an error if all futures ended with
/// an error; otherwise returns results of succesful futures and saves errors of the failed
/// futures.
pub async fn wait_all_failsafe<I>(
    category: &str,
    i: I,
) -> Result<Vec<<I::Item as TryFuture>::Ok>, <I::Item as TryFuture>::Error>
where
    I: IntoIterator,
    I::Item: TryFuture,
    <I::Item as Future>::Output:
        Into<Result<<I::Item as TryFuture>::Ok, <I::Item as TryFuture>::Error>>,
    <I::Item as TryFuture>::Error: std::fmt::Display,
{
    let mut oks = Vec::new();
    let mut errs = Vec::new();

    let output = futures::future::join_all(i).await;
    for item in output {
        match item.into() {
            Ok(ok) => oks.push(ok),
            Err(err) => {
                save_error(category, &err);
                errs.push(err)
            }
        }
    }

    if oks.is_empty() {
        match errs.into_iter().next() {
            Some(err) => return Err(err),
            None => return Ok(Vec::new()),
        }
    } else if errs.len() > ERRORS_CUTOFF {
        log::warn!(
            "A {} errors occurred during the `{}` execution.",
            errs.len(),
            category,
        );
    }

    Ok(oks)
}

/// Creates a future which represents either a collection of the results of the
/// futures given or an error.
///
/// But unlike the `try_wait_all_failsafe` method, it performs futures in chunks
/// to reduce descriptors usage.
pub async fn wait_all_failsafe_chunks<I>(
    category: &str,
    chunk_sizes: &[usize],
    i: I,
) -> Result<Vec<<I::Item as TryFuture>::Ok>, <I::Item as TryFuture>::Error>
where
    I: IntoIterator,
    I::Item: TryFuture,
    <I::Item as Future>::Output:
        Into<Result<<I::Item as TryFuture>::Ok, <I::Item as TryFuture>::Error>>,
    <I::Item as TryFuture>::Error: std::fmt::Display,
{
    let mut oks = Vec::new();
    let mut errs = Vec::new();
    for chunk in DynamicChunks::new(i, chunk_sizes) {
        let output = futures::future::join_all(chunk).await;
        for item in output {
            match item.into() {
                Ok(ok) => oks.push(ok),
                Err(err) => {
                    save_error(category, &err);
                    errs.push(err)
                }
            }
        }
    }

    if oks.is_empty() {
        match errs.into_iter().next() {
            Some(err) => return Err(err),
            None => return Ok(Vec::new()),
        }
    } else if errs.len() > ERRORS_CUTOFF {
        log::warn!(
            "A {} errors occurred during the `{}` execution.",
            errs.len(),
            category,
        );
    }

    Ok(oks)
}

/// An iterator similar to `.iter().chunks(..)`, but supporting multiple
/// different chunk sizes. Size of yielded batches is chosen one-by-one
/// from the provided list of sizes (preserving their order).
///
/// For example, if chunk sizes array is `[10, 20]` and the iterator is
/// created over an array of 43 elements, sizes of batches will be 10,
/// 20, 10 again and then 3 (remaining elements).
#[derive(Debug)]
pub struct DynamicChunks<T, I>
where
    I: Iterator<Item = T>,
{
    iterable: I,
    chunk_sizes: Vec<usize>,
    chunk_size_id: usize,
}

impl<T, I> DynamicChunks<T, I>
where
    I: Iterator<Item = T>,
{
    pub fn new<J>(iterable: J, chunk_sizes: &[usize]) -> Self
    where
        J: IntoIterator<Item = T, IntoIter = I>,
    {
        assert!(!chunk_sizes.is_empty());

        Self {
            iterable: iterable.into_iter(),
            chunk_sizes: chunk_sizes.to_vec(),
            chunk_size_id: 0,
        }
    }
}

impl<T, I> Iterator for DynamicChunks<T, I>
where
    I: Iterator<Item = T>,
{
    type Item = Vec<T>;

    fn next(&mut self) -> Option<Vec<T>> {
        let chunk_size = self.chunk_sizes[self.chunk_size_id];
        self.chunk_size_id = (self.chunk_size_id + 1) % self.chunk_sizes.len();

        let mut items = Vec::new();
        for _ in 0..chunk_size {
            if let Some(value) = self.iterable.next() {
                items.push(value);
            } else {
                break;
            }
        }

        if items.is_empty() {
            None
        } else {
            Some(items)
        }
    }
}

pub trait ResultEx<O, E> {
    fn split_errs(self) -> (Vec<O>, Vec<E>);

    fn collect_oks(self) -> Result<Vec<O>, E>
    where
        Self: Sized,
    {
        let (oks, errs) = self.split_errs();

        match errs.into_iter().next() {
            Some(err) => Err(err),
            None => Ok(oks),
        }
    }
}

impl<O, E> ResultEx<O, E> for Vec<Result<O, E>> {
    fn split_errs(self) -> (Vec<O>, Vec<E>) {
        let mut oks = Vec::with_capacity(self.len());
        let mut errs = Vec::with_capacity(self.len());

        for result in self {
            match result {
                Ok(ok) => oks.push(ok),
                Err(err) => errs.push(err),
            }
        }

        (oks, errs)
    }

    fn collect_oks(self) -> Result<Vec<O>, E>
    where
        Self: Sized,
    {
        self.into_iter().collect()
    }
}
