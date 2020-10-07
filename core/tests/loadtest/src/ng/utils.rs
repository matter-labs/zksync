//! Common functions shared by different scenarios.

// Built-in uses
use std::{
    iter::Iterator,
};
// External uses
use futures::{Future, TryFuture};
// Workspace uses
// Local uses

const CHUNK_SIZES: &[usize] = &[100];

pub async fn wait_all<I>(i: I) -> Vec<<I::Item as Future>::Output>
where
    I: IntoIterator,
    I::Item: Future,
{
    let mut output = Vec::new();
    for chunk in DynamicChunks::new(i, CHUNK_SIZES) {
        let values = futures::future::join_all(chunk).await;
        output.extend(values);
    }
    output
}

pub async fn try_wait_all<I>(
    i: I,
) -> Result<Vec<<I::Item as TryFuture>::Ok>, <I::Item as TryFuture>::Error>
where
    I: IntoIterator,
    I::Item: TryFuture,
{
    let mut output = Vec::new();
    for chunk in DynamicChunks::new(i, &CHUNK_SIZES) {
        output.extend(futures::future::try_join_all(chunk).await?);
    }
    Ok(output)
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
