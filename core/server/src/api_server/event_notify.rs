use actix::FinishStream;
use futures::{sync::oneshot, Future, Stream};
use models::{node::block::ExecutedOperations, Action, ActionType, Operation};
use std::collections::BTreeMap;
use storage::ConnectionPool;

pub enum EventSubscribe {
    Transaction {
        hash: Box<[u8; 32]>,
        commit: bool, // commit of verify
        notify: oneshot::Sender<()>,
    },
    PriorityOp {
        serial_id: u64,
        commit: bool,
        notify: oneshot::Sender<()>,
    },
}

enum BlockNotifierInput {
    NewOperationCommited(Operation),
    EventSubscription(EventSubscribe),
}

struct OperationNotifier {
    db_pool: ConnectionPool,

    tx_commit_subs: BTreeMap<[u8; 32], oneshot::Sender<()>>,
    prior_op_commit_subs: BTreeMap<u64, oneshot::Sender<()>>,

    prior_op_verify_subs: BTreeMap<u64, oneshot::Sender<()>>,
    tx_verify_subs: BTreeMap<[u8; 32], oneshot::Sender<()>>,
}

impl OperationNotifier {
    fn run<S: Stream<Item = BlockNotifierInput, Error = ()>>(
        mut self,
        input_stream: S,
    ) -> impl Future<Item = (), Error = ()> {
        input_stream
            .map(move |input| match input {
                BlockNotifierInput::EventSubscription(sub) => self.handle_subscription(sub),
                BlockNotifierInput::NewOperationCommited(op) => self.handle_new_block(op),
            })
            .finish()
    }

    // TODO: remove sub after timeout.
    fn handle_subscription(&mut self, new_sub: EventSubscribe) {
        match new_sub {
            EventSubscribe::Transaction {
                hash,
                commit,
                notify,
            } => {
                // Maybe tx was executed already.
                if let Some(receipt) = self
                    .db_pool
                    .access_storage()
                    .ok()
                    .and_then(|s| s.tx_receipt(hash.as_ref()).ok().unwrap_or(None))
                {
                    if commit {
                        notify.send(()).unwrap_or_default();
                        return;
                    } else {
                        if receipt.verified {
                            notify.send(()).unwrap_or_default();
                            return;
                        }
                    }
                }

                if commit {
                    self.tx_commit_subs.insert(*hash, notify);
                } else {
                    self.tx_verify_subs.insert(*hash, notify);
                }
            }
            EventSubscribe::PriorityOp {
                serial_id,
                commit,
                notify,
            } => {
                let executed_op = self.db_pool.access_storage().ok().and_then(|s| {
                    s.get_executed_priority_op(serial_id as u32)
                        .ok()
                        .unwrap_or(None)
                });
                if let Some(executed_op) = executed_op {
                    if commit {
                        notify.send(()).unwrap_or_default();
                        return;
                    } else {
                        if let Some(block_verify) =
                            self.db_pool.access_storage().ok().and_then(|s| {
                                s.load_stored_op_with_block_number(
                                    executed_op.block_number as u32,
                                    ActionType::VERIFY,
                                )
                            })
                        {
                            if block_verify.confirmed {
                                notify.send(()).unwrap_or_default();
                                return;
                            }
                        }
                    }
                }

                if commit {
                    self.prior_op_commit_subs.insert(serial_id, notify);
                } else {
                    self.prior_op_verify_subs.insert(serial_id, notify);
                }
            }
        }
    }
    fn handle_new_block(&mut self, op: Operation) {
        let commit = match &op.action {
            Action::Commit => true,
            Action::Verify { .. } => false,
        };

        for tx in op.block.block_transactions {
            match tx {
                ExecutedOperations::Tx(tx) => {
                    let hash = tx.tx.hash();
                    if commit {
                        self.tx_commit_subs
                            .remove(hash.as_ref())
                            .map(|n| n.send(()).unwrap_or_default());
                    } else {
                        self.tx_verify_subs
                            .remove(hash.as_ref())
                            .map(|n| n.send(()).unwrap_or_default());
                    }
                }
                ExecutedOperations::PriorityOp(op) => {
                    let id = op.priority_op.serial_id;
                    if commit {
                        self.prior_op_commit_subs
                            .remove(&id)
                            .map(|n| n.send(()).unwrap_or_default());
                    } else {
                        self.prior_op_verify_subs
                            .remove(&id)
                            .map(|n| n.send(()).unwrap_or_default());
                    }
                }
            }
        }
    }
}

pub fn start_sub_notifier<BStream, SStream>(
    db_pool: ConnectionPool,
    new_block_stream: BStream,
    subscription_stream: SStream,
) where
    BStream: Stream<Item = Operation, Error = ()> + 'static,
    SStream: Stream<Item = EventSubscribe, Error = ()> + 'static,
{
    let notifier = OperationNotifier {
        db_pool,
        tx_verify_subs: BTreeMap::new(),
        tx_commit_subs: BTreeMap::new(),
        prior_op_commit_subs: BTreeMap::new(),
        prior_op_verify_subs: BTreeMap::new(),
    };
    let input_stream = new_block_stream
        .map(BlockNotifierInput::NewOperationCommited)
        .select(subscription_stream.map(BlockNotifierInput::EventSubscription));
    actix::System::with_current(move |_| actix::spawn(notifier.run(input_stream)));
}
