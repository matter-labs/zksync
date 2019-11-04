use super::PriorityOpStatus;
use actix::FinishStream;
use futures::{
    sync::{mpsc, oneshot},
    Future, Sink as FuturesSink, Stream,
};
use im::HashMap;
use itertools::Itertools;
use jsonrpc_core::Params;
use jsonrpc_pubsub::{
    typed::{Sink, Subscriber},
    SubscriptionId,
};
use models::node::tx::TxHash;
use models::{
    node::block::ExecutedOperations,
    node::{Account, AccountAddress, AccountId, AccountUpdate},
    ActionType, Operation,
};
use std::collections::BTreeMap;
use storage::ConnectionPool;
use tokio::spawn;

use super::rpc_server::{ETHOpInfoResp, TransactionInfoResp};

const MAX_LISTENERS_PER_ENTITY: usize = 2048;

pub enum EventSubscribe {
    Transaction {
        hash: TxHash,
        action: ActionType,
        subscriber: Subscriber<TransactionInfoResp>,
    },
    PriorityOp {
        serial_id: u64,
        action: ActionType,
        subscriber: Subscriber<ETHOpInfoResp>,
    },
    Account {
        address: AccountAddress,
        action: ActionType,
        subscriber: Subscriber<Account>,
    },
}

pub enum EventNotifierRequest {
    Sub(EventSubscribe),
    Unsub(SubscriptionId),
}

enum BlockNotifierInput {
    NewOperationCommited(Operation),
    EventNotifiyRequest(EventNotifierRequest),
}

struct AccountSubscriptionState {
    account: Option<Account>,
    listeners: Vec<mpsc::Sender<Account>>,
}

struct SubscriptionSender<T> {
    id: SubscriptionId,
    sink: Sink<T>,
}

struct OperationNotifier {
    db_pool: ConnectionPool,
    tx_subs: BTreeMap<(TxHash, ActionType), Vec<SubscriptionSender<TransactionInfoResp>>>,
    prior_op_subs: BTreeMap<(u64, ActionType), Vec<SubscriptionSender<ETHOpInfoResp>>>,
    account_subs:
        BTreeMap<(AccountId, ActionType), (Option<Account>, Vec<SubscriptionSender<Account>>)>,
}

fn send_once<T: serde::Serialize>(sink: Sink<T>, val: T) {
    spawn(sink.notify(Ok(val)).map(drop).map_err(drop));
}

impl OperationNotifier {
    fn run<S: Stream<Item = BlockNotifierInput, Error = ()>>(
        mut self,
        input_stream: S,
    ) -> impl Future<Item = (), Error = ()> {
        input_stream
            .map(move |input| match input {
                BlockNotifierInput::EventNotifiyRequest(sub) => self.handle_notify_req(sub),
                BlockNotifierInput::NewOperationCommited(op) => self.handle_new_block(op),
            })
            .finish()
    }

    fn handle_notify_req(&mut self, new_sub: EventNotifierRequest) {
        match EventNotifierRequest {
            EventNotifierRequest::Sub(event_sub) => {
                let sub_result = match event_sub {
                    EventSubscribe::Transaction {
                        hash,
                        action,
                        subscriber,
                    } => self.handle_transaction_sub(hash, action, subscriber),
                    EventSubscribe::PriorityOp {
                        serial_id,
                        action,
                        subscriber,
                    } => self.handle_priority_op_sub(serial_id, action, subscriber),
                    EventSubscribe::Account {
                        address,
                        action,
                        subscriber,
                    } => self.handle_account_update_sub(address, action, subscriber),
                };

                if let Err(e) = sub_result {
                    warn!("Failed to subscribe for notification: {}", e);
                }
            }
            EventNotifierRequest::Unsub(sub_id) => {
                if let Some(sub) = self
                    .account_subs_known_id
                    .iter()
                    .find(|(id, _, _)| id == sub_id)
                    .clone()
                {
                    self.account_subs_known_id.remove(&sub.0);
                }
                // TODO: check if valid then copy paste
                unimplemented!();
            }
        }
    }

    fn handle_priority_op_sub(
        &mut self,
        serial_id: u64,
        action: ActionType,
        sub: Subscriber<ETHOpInfoResp>,
    ) -> Result<(), failure::Error> {
        let sub_id = SubscriptionId::String(format!("eosub{}", rand::thread_rng().get::<u64>()));

        // TODO: tmp
        let rec = ETHOpInfoResp {
            executed: true,
            block: None,
        };

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
                    let sink = sub.assign_id(sub_id)?;
                    // TODO:
                    send_once(sink, rec);
                    return Ok(());
                }
                ActionType::VERIFY => {
                    if let Some(block_verify) = storage.load_stored_op_with_block_number(
                        executed_op.block_number as u32,
                        ActionType::VERIFY,
                    ) {
                        if block_verify.confirmed {
                            let sink = sub.assign_id(sub_id)?;
                            // TODO:
                            send_once(sink, rec);
                            return Ok(());
                        }
                    }
                }
            }
        }

        let mut subs = self
            .prior_op_subs
            .remove(&(serial_id, action))
            .unwrap_or_default();
        if subs.len() < MAX_LISTENERS_PER_ENTITY {
            let sink = sub.assign_id(sub_id)?;
            subs.push(SubscriptionSender { id, sink });
        };
        self.prior_op_subs.insert((serial_id, action), subs);
        Ok(())
    }

    fn handle_transaction_sub(
        &mut self,
        hash: TxHash,
        action: ActionType,
        sub: Subscriber<TransactionInfoResp>,
    ) -> Result<(), failure::Error> {
        // Maybe tx was executed already.
        let receipt = self.db_pool.access_storage()?.tx_receipt(hash.as_ref())?;
        // TODO: change to good.
        let rec = TransactionInfoResp {
            executed: true,
            success: None,
            fail_reason: None,
            block: None,
        };
        let id = SubscriptionId::String(format!("txsub{}", rand::thread_rng().get::<u64>()));

        if let Some(receipt) = receipt {
            match action {
                ActionType::COMMIT => {
                    let sink = sub.assign_id(id)?;
                    // TODO:
                    send_once(sink, rec);
                    return Ok(());
                }
                ActionType::VERIFY => {
                    if receipt.verified {
                        let sink = sub.assign_id(id)?;
                        // TODO:
                        send_once(sink, rec);
                        return Ok(());
                    }
                }
            }
        }

        let mut subs = self.tx_subs.remove(&(hash, action)).unwrap_or_default();
        if subs.len() < MAX_LISTENERS_PER_ENTITY {
            let sink = sub.assign_id(id)?;
            subs.push(SubscriptionSender { id, sink });
        }
        self.tx_subs
            .insert((hash, action), SubscriptionSender { id, sink });
        Ok(())
    }

    fn handle_account_update_sub(
        &mut self,
        address: AccountAddress,
        action: ActionType,
        sub: Subscriber<Account>,
    ) -> Result<(), failure::Error> {
        let account_state = self
            .db_pool
            .access_storage()?
            .account_state_by_address(&address)?;

        let account_id = if let Some(id) = account_state.commited.map(|(id, _)| id) {
            id
        } else {
            bail!("AccountId is unkwown");
        };

        let sub_id = SubscriptionId::String(format!("acsub{}", rand::thread_rng().get::<u64>()));

        let account_state = match action {
            ActionType::COMMIT => account_state.commited,
            ActionType::VERIFY => account_state.verified,
        }
        .map(|(_, a)| a);

        let (acc, mut subs) = self
            .account_subs
            .remove(&(account_id, action))
            .unwrap_or_default();
        if subs.len() < MAX_LISTENERS_PER_ENTITY {
            let account_state = if acc.is_some() { acc } else { account_state };

            let sink = sub.assign_id(sub_id)?;

            let initial_account_state = account_state
                .clone()
                .unwrap_or_else(|| Account::default_with_address(&address));
            // TODO: SEND INITIAL ACCOUNT STATE
            send_once(sink.clone(), initial_account_state);

            subs.push(SubscriptionSender { id: sub_id, sink });
        }

        self.account_subs
            .insert((account_id, action), (account_state, subs));
        Ok(())
    }

    fn handle_new_block(&mut self, op: Operation) {
        let action = op.action.get_type();

        for tx in op.block.block_transactions {
            match tx {
                ExecutedOperations::Tx(tx) => {
                    let hash = tx.tx.hash();
                    if let Some(subs) = self.tx_subs.remove(&(*hash, action)) {
                        // TODO: change to good.
                        let rec = TransactionInfoResp {
                            executed: true,
                            success: None,
                            fail_reason: None,
                            block: None,
                        };
                        for sub in subs {
                            send_once(sub.sink, rec.clone());
                        }
                    }
                }
                ExecutedOperations::PriorityOp(prior_op) => {
                    let id = prior_op.priority_op.serial_id;
                    if let Some(sink) = self.prior_op_subs.remove(&(id, action)) {
                        // TODO: tmp
                        let rec = ETHOpInfoResp {
                            executed: true,
                            block: None,
                        };
                        for sub in subs {
                            send_once(sub.sink, rec.clone());
                        }
                    }
                }
            }
        }

        let mut updates = op.accounts_updated;
        updates.sort_by_key(|(id, _)| *id);
        let updates = updates
            .into_iter()
            .group_by(|(id, _)| *id)
            .into_iter()
            .map(|(id, grouped_updates)| {
                let acc_updates = grouped_updates.map(|(_, u)| u).collect::<Vec<_>>();
                (id, acc_updates)
            })
            .collect::<Vec<_>>();

        for (id, updates) in updates.into_iter() {
            if let Some((acc, subs)) = self.account_subs.remove(&(id, action)) {
                if let Some(account) = Account::apply_updates(acc, &updates) {
                    for sub in &subs {
                        sub.sink.notify(Ok(account));
                    }
                    self.account_subs
                        .insert(&(id, action), (Some(account), sub));
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
    SStream: Stream<Item = EventNotifierRequest, Error = ()> + 'static,
{
    let notifier = OperationNotifier {
        db_pool,
        tx_subs: BTreeMap::new(),
        prior_op_subs: BTreeMap::new(),
        account_subs: BTreeMap::new(),
    };
    let input_stream = new_block_stream
        .map(BlockNotifierInput::NewOperationCommited)
        .select(subscription_stream.map(BlockNotifierInput::EventNotifiyRequest));
    spawn(move || notifier.run(input_stream));
}
