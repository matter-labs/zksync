use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};
use zksync_types::mempool::SignedTxVariant;

#[derive(Debug, Clone)]
struct MempoolPendingTransaction {
    valid_from: u64,
    tx: SignedTxVariant,
}

impl Eq for MempoolPendingTransaction {}

impl PartialEq for MempoolPendingTransaction {
    fn eq(&self, other: &Self) -> bool {
        self.tx.hashes() == other.tx.hashes()
    }
}

impl Ord for MempoolPendingTransaction {
    fn cmp(&self, other: &Self) -> Ordering {
        // We will compare pending transactions by their `valid_from` value to use the earliest one
        other
            .valid_from
            .cmp(&self.valid_from)
            .then_with(|| self.tx.hashes().cmp(&other.tx.hashes()))
    }
}

impl PartialOrd for MempoolPendingTransaction {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone)]
pub struct MempoolTransactionsQueue {
    // transactions ready for execution
    ready_txs: VecDeque<SignedTxVariant>,
    // transactions that are not ready yet because of the `valid_from` field
    pending_txs: BinaryHeap<MempoolPendingTransaction>,
}

impl MempoolTransactionsQueue {
    pub fn new() -> Self {
        Self {
            ready_txs: VecDeque::new(),
            pending_txs: BinaryHeap::new(),
        }
    }

    pub fn pop_front(&mut self) -> Option<SignedTxVariant> {
        self.ready_txs.pop_front()
    }

    pub fn push_front(&mut self, tx: SignedTxVariant) {
        self.ready_txs.push_front(tx);
    }

    pub fn add_tx_variant(&mut self, tx: SignedTxVariant) {
        self.pending_txs.push(MempoolPendingTransaction {
            valid_from: tx
                .get_transactions()
                .into_iter()
                .map(|tx| tx.tx.valid_from())
                .max()
                .unwrap_or(0),
            tx,
        });
    }

    pub fn prepare_new_ready_transactions(&mut self, block_timestamp: u64) {
        // Move some pending transactions to the ready_txs queue
        let mut ready_pending_transactions = {
            let mut ready_pending_transactions = Vec::new();

            while let Some(pending_tx) = self.pending_txs.peek() {
                if pending_tx.valid_from <= block_timestamp {
                    ready_pending_transactions.push(pending_tx.tx.clone());
                    self.pending_txs.pop();
                } else {
                    break;
                }
            }

            // Now transactions should be sorted by the nonce (transaction natural order)
            // According to our convention in batch `fee transaction` would be the last one, so we would use nonce from it as a key for sort
            ready_pending_transactions.sort_by_key(|tx| match tx {
                SignedTxVariant::Tx(tx) => tx.tx.nonce(),
                SignedTxVariant::Batch(batch) => batch
                    .txs
                    .last()
                    .expect("batch must contain at least one transaction")
                    .tx
                    .nonce(),
            });

            VecDeque::<SignedTxVariant>::from(ready_pending_transactions)
        };

        self.ready_txs.append(&mut ready_pending_transactions);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mempool::Address;
    use zksync_types::tx::{TimeRange, Transfer, Withdraw};
    use zksync_types::{AccountId, Nonce, SignedZkSyncTx, TokenId, ZkSyncTx};

    fn get_transfer_with_timestamps(valid_from: u64, valid_until: u64) -> SignedTxVariant {
        let transfer = Transfer::new(
            AccountId(4242),
            Address::random(),
            Address::random(),
            TokenId(0),
            500u32.into(),
            20u32.into(),
            Nonce(11),
            TimeRange::new(valid_from, valid_until),
            None,
        );

        SignedTxVariant::Tx(SignedZkSyncTx {
            tx: ZkSyncTx::Transfer(Box::new(transfer)),
            eth_sign_data: None,
        })
    }

    fn get_withdraw() -> SignedTxVariant {
        let withdraw = Withdraw::new(
            AccountId(3),
            "7777777777777777777777777777777777777777".parse().unwrap(),
            [9u8; 20].into(),
            TokenId(1),
            20u32.into(),
            10u32.into(),
            Nonce(2),
            Default::default(),
            None,
        );

        SignedTxVariant::Tx(SignedZkSyncTx {
            tx: ZkSyncTx::Withdraw(Box::new(withdraw)),
            eth_sign_data: None,
        })
    }

    #[test]
    fn test_mempool_transactions_queue() {
        let mut transactions_queue = MempoolTransactionsQueue {
            ready_txs: VecDeque::new(),
            pending_txs: BinaryHeap::new(),
        };

        let withdraw0 = get_withdraw();
        let transfer1 = get_transfer_with_timestamps(5, 13);
        let transfer2 = get_transfer_with_timestamps(10, 15);

        // Insert transactions to the mempool transcations queue
        {
            transactions_queue.add_tx_variant(withdraw0.clone());
            assert_eq!(transactions_queue.pending_txs.peek().unwrap().valid_from, 0);

            // Some "random" order for trancsactions
            transactions_queue.add_tx_variant(transfer2.clone());
            transactions_queue.add_tx_variant(transfer1.clone());
        }

        // At first we should have only one transaction ready
        {
            transactions_queue.prepare_new_ready_transactions(3);

            assert_eq!(transactions_queue.ready_txs.len(), 1);
            assert_eq!(transactions_queue.ready_txs[0].hashes(), withdraw0.hashes());
        }

        // One more transaction is ready
        {
            transactions_queue.prepare_new_ready_transactions(9);

            assert_eq!(transactions_queue.ready_txs.len(), 2);
            assert_eq!(transactions_queue.ready_txs[1].hashes(), transfer1.hashes());
        }

        // The last one is ready
        {
            transactions_queue.prepare_new_ready_transactions(10);

            assert_eq!(transactions_queue.ready_txs.len(), 3);
            assert_eq!(transactions_queue.ready_txs[2].hashes(), transfer2.hashes());
        }
    }
}
