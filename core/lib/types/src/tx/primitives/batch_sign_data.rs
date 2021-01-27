// External uses
use anyhow::ensure;
use itertools::Itertools;
// Workspace uses
use zksync_basic_types::Address;
// Local uses
use super::eth_signature::TxEthSignature;
use crate::{Token, ZkSyncTx};

/// Encapsulates transactions batch signature data. Should only be created via `new()`
/// as long as errors are possible.
#[derive(Debug, Clone)]
pub struct BatchSignData {
    pub signatures: Vec<TxEthSignature>,
    pub message: Vec<u8>,
}

impl BatchSignData {
    /// Construct the message user is expected to sign for the given batch and pack
    /// it along with signatures. Since there can be multiple senders in a single batch,
    /// separate them with
    ///
    /// `From: {address}`
    pub fn new(
        txs: Vec<(ZkSyncTx, Token, Address)>,
        signatures: Vec<TxEthSignature>,
    ) -> anyhow::Result<BatchSignData> {
        ensure!(!txs.is_empty(), "Transaction batch cannot be empty");

        let message = BatchSignData::get_batch_sign_message(txs);

        Ok(BatchSignData {
            signatures,
            message,
        })
    }

    /// Construct the message user is expected to sign for the given batch.
    pub fn get_batch_sign_message(txs: Vec<(ZkSyncTx, Token, Address)>) -> Vec<u8> {
        let grouped = txs.into_iter().group_by(|tx| tx.2);
        let mut iter = grouped.into_iter().peekable();
        // The message is empty if there're no transactions.
        let first = match iter.next() {
            Some(group) => group,
            None => return Vec::new(),
        };
        // Check whether there're mutiple addresses in the batch, concatenate their
        // transaction messages with `From: {address}` separator.
        // Otherwise, process the whole group at once.
        match iter.peek() {
            Some(_) => {
                let head = BatchSignData::group_message(first.1, Some(first.0));
                let tail = itertools::join(
                    iter.map(|(address, group)| BatchSignData::group_message(group, Some(address))),
                    "\n\n",
                );
                format!("{}\n\n{}", head, tail)
            }
            None => BatchSignData::group_message(first.1, None),
        }
        .into_bytes()
    }

    fn group_message<I>(iter: I, address: Option<Address>) -> String
    where
        I: IntoIterator<Item = (ZkSyncTx, Token, Address)>,
    {
        let mut iter = iter.into_iter().peekable();
        // The group is not empty.
        let nonce = iter.peek().unwrap().0.nonce();
        let message = itertools::join(
            iter.filter_map(|(tx, token, _)| tx.get_ethereum_sign_message_part(token))
                .filter(|part| !part.is_empty()),
            "\n",
        );
        let body = format!(
            "{message}\n\
            Nonce: {nonce}",
            message = message,
            nonce = nonce
        );
        match address {
            Some(address) => format!(
                "From: 0x{address}\n\
                {body}",
                address = hex::encode(address),
                body = body
            ),
            None => body,
        }
    }
}
