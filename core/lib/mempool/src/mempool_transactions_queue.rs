use crate::MempoolState;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};
use zksync_types::mempool::SignedTxVariant;
use zksync_types::tx::error::TxAddError;
use zksync_types::PriorityOp;

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
pub(crate) struct MempoolTransactionsQueue {
    /// Transactions ready for execution.
    ready_l2_transactions: VecDeque<SignedTxVariant>,
    /// Transactions that are not ready yet because of the `valid_from` field.
    pending_l2_transactions: BinaryHeap<MempoolPendingTransaction>,

    l1_transactions: VecDeque<PriorityOp>,
}

impl MempoolTransactionsQueue {
    pub(crate) fn new(
        l1_transactions: VecDeque<PriorityOp>,
        l2_transactions: VecDeque<SignedTxVariant>,
    ) -> Self {
        let mut res = Self {
            ready_l2_transactions: Default::default(),
            pending_l2_transactions: Default::default(),
            l1_transactions,
        };
        // Due to complexity of json structure in database for transactions it's easier and safer
        // to add even not ready txs to mempool and prepare them before when it's needed.
        for tx in l2_transactions {
            res.add_l2_transaction(tx)
        }
        res
    }

    fn pop_l2_transactions_front(&mut self) -> Option<SignedTxVariant> {
        self.ready_l2_transactions.pop_front()
    }

    fn pop_front_l1_transactions(&mut self) -> Option<PriorityOp> {
        self.l1_transactions.pop_front()
    }

    #[allow(dead_code)]
    fn add_l1_transactions(&mut self, mut ops: Vec<PriorityOp>) {
        ops.sort_unstable_by_key(|key| key.serial_id);
        for op in ops {
            self.l1_transactions.push_back(op);
        }
    }

    fn add_l2_transaction(&mut self, tx: SignedTxVariant) {
        self.pending_l2_transactions
            .push(MempoolPendingTransaction {
                valid_from: tx
                    .get_transactions()
                    .into_iter()
                    .map(|tx| tx.tx.valid_from())
                    .max()
                    .unwrap_or(0),
                tx,
            });
    }

    fn prepare_new_ready_l2_transactions(&mut self, block_timestamp: u64) {
        // Move some pending transactions to the ready_txs queue
        let mut ready_pending_l2_operations = {
            let mut ready_pending_l2_operations = VecDeque::new();

            while let Some(pending_tx) = self.pending_l2_transactions.peek() {
                if pending_tx.valid_from <= block_timestamp {
                    ready_pending_l2_operations.push_back(pending_tx.tx.clone());
                    self.pending_l2_transactions.pop();
                } else {
                    break;
                }
            }
            ready_pending_l2_operations
        };

        // Now transactions should be sorted by the nonce (transaction natural order)
        // According to our convention in batch `fee transaction` would be the last one, so we would use nonce from it as a key for sort
        self.ready_l2_transactions
            .append(&mut ready_pending_l2_operations);
        self.ready_l2_transactions
            .make_contiguous()
            .sort_by_key(|tx| match tx {
                SignedTxVariant::Tx(tx) => tx.tx.nonce(),
                SignedTxVariant::Batch(batch) => batch
                    .txs
                    .last()
                    .expect("batch must contain at least one transaction")
                    .tx
                    .nonce(),
            });
    }

    /// Collect txs depending on desired chunks and execution time
    pub(crate) async fn select_transactions(
        &mut self,
        chunks: usize,
        current_unprocessed_priority_op: u64,
        block_timestamp: u64,
        mempool_state: &MempoolState,
    ) -> Result<(Vec<SignedTxVariant>, Vec<PriorityOp>, usize), TxAddError> {
        let (chunks_left, priority_ops) =
            self.select_l1_transactions(chunks, current_unprocessed_priority_op);

        let (chunks_left, executed_txs) = self
            .select_l2_transactions(chunks_left, block_timestamp, mempool_state)
            .await?;

        Ok((executed_txs, priority_ops, chunks_left))
    }

    /// Returns: chunks left from max amount of chunks, ops selected
    fn select_l1_transactions(
        &mut self,
        max_block_size_chunks: usize,
        current_unprocessed_l1_tx: u64,
    ) -> (usize, Vec<PriorityOp>) {
        let mut result = vec![];

        let mut used_chunks = 0;
        let mut current_l1_tx = current_unprocessed_l1_tx;
        while let Some(tx) = self.pop_front_l1_transactions() {
            // Since the transaction addition is asynchronous process and we are checking node many times,
            // We can find some already processed priority ops
            if tx.serial_id < current_l1_tx {
                vlog::warn!("Already processed priority op was found in queue");
                // We can skip already processed priority operations
                continue;
            }
            assert_eq!(current_l1_tx, tx.serial_id, "Wrong order for priority ops");
            if used_chunks + tx.data.chunks() <= max_block_size_chunks {
                used_chunks += tx.data.chunks();
                result.push(tx);
                current_l1_tx += 1;
            } else {
                // We don't push back transactions because the transaction queue is used only once
                break;
            }
        }
        (max_block_size_chunks - used_chunks, result)
    }

    /// Collect txs depending on the remaining chunks size
    async fn select_l2_transactions(
        &mut self,
        mut chunks_left: usize,
        block_timestamp: u64,
        mempool_state: &MempoolState,
    ) -> Result<(usize, Vec<SignedTxVariant>), TxAddError> {
        self.prepare_new_ready_l2_transactions(block_timestamp);

        let mut txs_for_commit = Vec::new();

        while let Some(tx) = self.pop_l2_transactions_front() {
            let chunks_for_tx = mempool_state.required_chunks(&tx).await?;
            if chunks_left >= chunks_for_tx {
                txs_for_commit.push(tx);
                chunks_left -= chunks_for_tx;
            } else {
                // We don't push back transactions because the transaction queue is used only once
                break;
            }
        }
        Ok((chunks_left, txs_for_commit))
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;

    use zksync_types::tx::{TimeRange, Transfer, Withdraw};
    use zksync_types::{
        AccountId, Address, Deposit, Nonce, SignedZkSyncTx, TokenId, ZkSyncPriorityOp, ZkSyncTx,
    };

    use super::*;

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
            created_at: Utc::now(),
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
            created_at: Utc::now(),
        })
    }

    #[test]
    fn test_priority_queue() {
        let mut transactions_queue = MempoolTransactionsQueue {
            ready_l2_transactions: VecDeque::new(),
            pending_l2_transactions: BinaryHeap::new(),
            l1_transactions: Default::default(),
        };

        transactions_queue.add_l1_transactions(vec![
            PriorityOp {
                serial_id: 3,
                data: ZkSyncPriorityOp::Deposit(Deposit {
                    from: Default::default(),
                    token: Default::default(),
                    amount: Default::default(),
                    to: Default::default(),
                }),
                deadline_block: 0,
                eth_hash: Default::default(),
                eth_block: 0,
                eth_block_index: None,
            },
            PriorityOp {
                serial_id: 1,
                data: ZkSyncPriorityOp::Deposit(Deposit {
                    from: Default::default(),
                    token: Default::default(),
                    amount: Default::default(),
                    to: Default::default(),
                }),
                deadline_block: 0,
                eth_hash: Default::default(),
                eth_block: 0,
                eth_block_index: None,
            },
            PriorityOp {
                serial_id: 2,
                data: ZkSyncPriorityOp::Deposit(Deposit {
                    from: Default::default(),
                    token: Default::default(),
                    amount: Default::default(),
                    to: Default::default(),
                }),
                deadline_block: 0,
                eth_hash: Default::default(),
                eth_block: 0,
                eth_block_index: None,
            },
        ]);
        transactions_queue.add_l1_transactions(vec![
            PriorityOp {
                serial_id: 4,
                data: ZkSyncPriorityOp::Deposit(Deposit {
                    from: Default::default(),
                    token: Default::default(),
                    amount: Default::default(),
                    to: Default::default(),
                }),
                deadline_block: 0,
                eth_hash: Default::default(),
                eth_block: 0,
                eth_block_index: None,
            },
            PriorityOp {
                serial_id: 6,
                data: ZkSyncPriorityOp::Deposit(Deposit {
                    from: Default::default(),
                    token: Default::default(),
                    amount: Default::default(),
                    to: Default::default(),
                }),
                deadline_block: 0,
                eth_hash: Default::default(),
                eth_block: 0,
                eth_block_index: None,
            },
            PriorityOp {
                serial_id: 5,
                data: ZkSyncPriorityOp::Deposit(Deposit {
                    from: Default::default(),
                    token: Default::default(),
                    amount: Default::default(),
                    to: Default::default(),
                }),
                deadline_block: 0,
                eth_hash: Default::default(),
                eth_block: 0,
                eth_block_index: None,
            },
        ]);
        let op = transactions_queue.pop_front_l1_transactions().unwrap();
        assert_eq!(op.serial_id, 1);
        let op = transactions_queue.pop_front_l1_transactions().unwrap();
        assert_eq!(op.serial_id, 2);
        let op = transactions_queue.pop_front_l1_transactions().unwrap();
        assert_eq!(op.serial_id, 3);
        let op = transactions_queue.pop_front_l1_transactions().unwrap();
        assert_eq!(op.serial_id, 4);
        let op = transactions_queue.pop_front_l1_transactions().unwrap();
        assert_eq!(op.serial_id, 5);
        let op = transactions_queue.pop_front_l1_transactions().unwrap();
        assert_eq!(op.serial_id, 6);
    }

    #[test]
    fn test_mempool_transactions_queue() {
        let mut transactions_queue = MempoolTransactionsQueue {
            ready_l2_transactions: VecDeque::new(),
            pending_l2_transactions: BinaryHeap::new(),
            l1_transactions: Default::default(),
        };

        let withdraw0 = get_withdraw();
        let transfer1 = get_transfer_with_timestamps(5, 13);
        let transfer2 = get_transfer_with_timestamps(10, 15);

        // Insert transactions to the mempool transcations queue
        {
            transactions_queue.add_l2_transaction(withdraw0.clone());
            assert_eq!(
                transactions_queue
                    .pending_l2_transactions
                    .peek()
                    .unwrap()
                    .valid_from,
                0
            );

            // Some "random" order for trancsactions
            transactions_queue.add_l2_transaction(transfer2.clone());
            transactions_queue.add_l2_transaction(transfer1.clone());
        }

        // At first we should have only one transaction ready
        {
            transactions_queue.prepare_new_ready_l2_transactions(3);

            assert_eq!(transactions_queue.ready_l2_transactions.len(), 1);
            assert_eq!(
                transactions_queue.ready_l2_transactions[0].hashes(),
                withdraw0.hashes()
            );
        }

        // One more transaction is ready
        {
            transactions_queue.prepare_new_ready_l2_transactions(9);

            assert_eq!(transactions_queue.ready_l2_transactions.len(), 2);
            assert_eq!(
                transactions_queue.ready_l2_transactions[1].hashes(),
                transfer1.hashes()
            );
        }

        // The last one is ready
        {
            transactions_queue.prepare_new_ready_l2_transactions(10);

            assert_eq!(transactions_queue.ready_l2_transactions.len(), 3);
            assert_eq!(
                transactions_queue.ready_l2_transactions[2].hashes(),
                transfer2.hashes()
            );
        }
    }
}
