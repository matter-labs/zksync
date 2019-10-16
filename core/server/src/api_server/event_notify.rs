use super::PriorityOpStatus;
use actix::FinishStream;
use futures::{
    sync::{mpsc, oneshot},
    Future, Stream,
};
use models::{
    node::block::ExecutedOperations,
    node::{Account, AccountAddress, AccountId, AccountUpdate},
    ActionType, Operation,
};
use std::collections::BTreeMap;
use storage::{ConnectionPool, TxReceiptResponse};

const MAX_LISTENERS_PER_ENTITY: usize = 4096;

pub enum EventSubscribe {
    Transaction {
        hash: Box<[u8; 32]>,
        action: ActionType,
        notify: oneshot::Sender<TxReceiptResponse>,
    },
    PriorityOp {
        serial_id: u64,
        action: ActionType,
        notify: oneshot::Sender<PriorityOpStatus>,
    },
}

enum BlockNotifierInput {
    NewOperationCommited(Operation),
    EventSubscription(EventSubscribe),
}

struct AccountSubscriptionState {
    account: Option<Account>,
    listeners: Vec<mpsc::Sender<Account>>,
}

struct OperationNotifier {
    db_pool: ConnectionPool,
    /// (tx_hash, action) -> subscriber channels
    tx_subs: BTreeMap<([u8; 32], ActionType), Vec<oneshot::Sender<TxReceiptResponse>>>,
    /// (tx_hash, action) -> subscriber channels
    prior_op_subs: BTreeMap<(u64, ActionType), Vec<oneshot::Sender<PriorityOpStatus>>>,

    account_subs_unknown_id: BTreeMap<(AccountAddress, ActionType), AccountSubscriptionState>,
    account_subs_known_id: BTreeMap<(AccountId, ActionType), AccountSubscriptionState>,
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
        let sub_result = match new_sub {
            EventSubscribe::Transaction {
                hash,
                action,
                notify,
            } => self.handle_transaction_sub(hash, action, notify),
            EventSubscribe::PriorityOp {
                serial_id,
                action,
                notify,
            } => self.handle_priority_op_sub(serial_id, action, notify),
        };

        if let Err(e) = sub_result {
            warn!("Failed to subscribe for notification: {}", e);
        }
    }

    fn handle_priority_op_sub(
        &mut self,
        serial_id: u64,
        action: ActionType,
        notify: oneshot::Sender<PriorityOpStatus>,
    ) -> Result<(), failure::Error> {
        // Maybe it was executed already
        let storage = self.db_pool.access_storage()?;
        let executed_op = storage.get_executed_priority_op(serial_id as u32)?;
        if let Some(executed_op) = executed_op {
            let prior_op_status = PriorityOpStatus {
                executed: true,
                block: Some(executed_op.block_number),
            };
            match action {
                ActionType::COMMIT => {
                    notify.send(prior_op_status).unwrap_or_default();
                    return Ok(());
                }
                ActionType::VERIFY => {
                    if let Some(block_verify) = storage.load_stored_op_with_block_number(
                        executed_op.block_number as u32,
                        ActionType::VERIFY,
                    ) {
                        if block_verify.confirmed {
                            notify.send(prior_op_status).unwrap_or_default();
                            return Ok(());
                        }
                    }
                }
            }
        }

        let mut listeners = self
            .prior_op_subs
            .remove(&(serial_id, action))
            .unwrap_or_default();
        if listeners.len() < MAX_LISTENERS_PER_ENTITY {
            listeners.push(notify);
        }
        self.prior_op_subs.insert((serial_id, action), listeners);

        Ok(())
    }

    fn handle_transaction_sub(
        &mut self,
        hash: Box<[u8; 32]>,
        action: ActionType,
        notify: oneshot::Sender<TxReceiptResponse>,
    ) -> Result<(), failure::Error> {
        // Maybe tx was executed already.
        let receipt = self.db_pool.access_storage()?.tx_receipt(hash.as_ref())?;
        if let Some(receipt) = receipt {
            match action {
                ActionType::COMMIT => {
                    notify.send(receipt).unwrap_or_default();
                    return Ok(());
                }
                ActionType::VERIFY => {
                    if receipt.verified {
                        notify.send(receipt).unwrap_or_default();
                        return Ok(());
                    }
                }
            }
        }

        let mut listeners = self.tx_subs.remove(&(*hash, action)).unwrap_or_default();
        if listeners.len() < MAX_LISTENERS_PER_ENTITY {
            listeners.push(notify);
        }
        self.tx_subs.insert((*hash, action), listeners);
        Ok(())
    }

    fn handle_account_update_sub(
        &mut self,
        address: AccountAddress,
        action: ActionType,
        notify: mpsc::Sender<Account>,
    ) -> Result<(), failure::Error> {
        let account_state = self
            .db_pool
            .access_storage()?
            .account_state_by_address(&address)?;

        let resolved_account = match action {
            ActionType::COMMIT => account_state.commited,
            ActionType::VERIFY => account_state.verified,
        };

        if let Some((resolved_id, account)) = resolved_account {
            let mut subscription = self
                .account_subs_known_id
                .remove(&(resolved_id, action))
                .unwrap_or_else(|| AccountSubscriptionState {
                    account: Some(account),
                    listeners: Vec::new(),
                });
            if subscription.listeners.len() < MAX_LISTENERS_PER_ENTITY {
                subscription.listeners.push(notify);
            }
            self.account_subs_known_id
                .insert((resolved_id, action), subscription);
        } else {
            let mut subscription = self
                .account_subs_unknown_id
                .remove(&(address.clone(), action))
                .unwrap_or_else(|| AccountSubscriptionState {
                    account: None,
                    listeners: Vec::new(),
                });

            if subscription.listeners.len() < MAX_LISTENERS_PER_ENTITY {
                subscription.listeners.push(notify);
            }

            self.account_subs_unknown_id
                .insert((address, action), subscription);
        }

        Ok(())
    }

    fn handle_new_block(&mut self, op: Operation) {
        let action = op.action.get_type();

        for tx in op.block.block_transactions {
            match tx {
                ExecutedOperations::Tx(tx) => {
                    let hash = tx.tx.hash();
                    let subs = self.tx_subs.remove(&(*hash, action));
                    if let Some(channels) = subs {
                        let receipt = TxReceiptResponse {
                            tx_hash: hex::encode(hash.as_ref()),
                            block_number: op.block.block_number as i64,
                            success: tx.success,
                            fail_reason: tx.fail_reason,
                            verified: op.action.get_type() == ActionType::VERIFY,
                            prover_run: None,
                        };
                        for ch in channels {
                            ch.send(receipt.clone()).unwrap_or_default();
                        }
                    }
                }
                ExecutedOperations::PriorityOp(prior_op) => {
                    let id = prior_op.priority_op.serial_id;
                    let subs = self.prior_op_subs.remove(&(id, action));
                    if let Some(channels) = subs {
                        let prior_op_status = PriorityOpStatus {
                            executed: true,
                            block: Some(op.block.block_number as i64),
                        };

                        for ch in channels {
                            ch.send(prior_op_status.clone()).unwrap_or_default();
                        }
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
        tx_subs: BTreeMap::new(),
        prior_op_subs: BTreeMap::new(),
        account_subs_known_id: BTreeMap::new(),
        account_subs_unknown_id: BTreeMap::new(),
    };
    let input_stream = new_block_stream
        .map(BlockNotifierInput::NewOperationCommited)
        .select(subscription_stream.map(BlockNotifierInput::EventSubscription));
    actix::System::with_current(move |_| actix::spawn(notifier.run(input_stream)));
}
