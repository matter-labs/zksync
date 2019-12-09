use super::rpc_server::{ETHOpInfoResp, TransactionInfoResp};
use crate::api_server::rpc_server::{BlockInfo, ResponseAccountState};
use crate::ThreadPanicNotify;
use actix::FinishStream;
use failure::{bail, format_err};
use futures::{Future, Stream};
use jsonrpc_pubsub::{
    typed::{Sink, Subscriber},
    SubscriptionId,
};
use models::node::tx::TxHash;
use models::{
    node::block::ExecutedOperations,
    node::{AccountAddress, AccountId},
    ActionType, Operation,
};
use std::collections::BTreeMap;
use storage::ConnectionPool;
use tokio::spawn;

const MAX_LISTENERS_PER_ENTITY: usize = 2048;
const TX_SUB_PREFIX: &str = "txsub";
const ETHOP_SUB_PREFIX: &str = "eosub";
const ACCOUNT_SUB_PREFIX: &str = "acsub";

pub enum EventSubscribeRequest {
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
        subscriber: Subscriber<ResponseAccountState>,
    },
}

pub enum EventNotifierRequest {
    Sub(EventSubscribeRequest),
    Unsub(SubscriptionId),
}

enum BlockNotifierInput {
    NewOperationCommitted(Operation),
    EventNotifyRequest(EventNotifierRequest),
}

struct SubscriptionSender<T> {
    id: SubscriptionId,
    sink: Sink<T>,
}

struct OperationNotifier {
    db_pool: ConnectionPool,
    tx_subs: BTreeMap<(TxHash, ActionType), Vec<SubscriptionSender<TransactionInfoResp>>>,
    prior_op_subs: BTreeMap<(u64, ActionType), Vec<SubscriptionSender<ETHOpInfoResp>>>,
    account_subs: BTreeMap<(AccountId, ActionType), Vec<SubscriptionSender<ResponseAccountState>>>,
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
            .map(move |input| {
                let res = match input {
                    BlockNotifierInput::EventNotifyRequest(sub) => self
                        .handle_notify_req(sub)
                        .map_err(|e| format_err!("Failed to handle sub request: {}", e)),
                    BlockNotifierInput::NewOperationCommitted(op) => self
                        .handle_new_block(op)
                        .map_err(|e| format_err!("Failed to handle new block: {}", e)),
                };
                if let Err(e) = res {
                    warn!("Notifier error: {}", e);
                }
            })
            .finish()
    }

    fn handle_unsub(&mut self, sub_id: SubscriptionId) -> Result<(), failure::Error> {
        let str_sub_id = if let SubscriptionId::String(str_sub_id) = sub_id.clone() {
            str_sub_id
        } else {
            bail!("SubsriptionId should be String");
        };
        let incorrect_id_err = || format_err!("Incorrect id: {:?}", str_sub_id);
        let mut id_split = str_sub_id.split('/').collect::<Vec<&str>>().into_iter();
        let sub_type = id_split.next().ok_or_else(incorrect_id_err)?;
        let sub_unique_id = id_split.next().ok_or_else(incorrect_id_err)?;
        let sub_action = id_split.next().ok_or_else(incorrect_id_err)?;

        let sub_action: ActionType = sub_action.parse().map_err(|_| incorrect_id_err())?;
        match sub_type {
            ETHOP_SUB_PREFIX => {
                let serial_id: u64 = sub_unique_id.parse()?;
                if let Some(mut subs) = self.prior_op_subs.remove(&(serial_id, sub_action)) {
                    subs.retain(|sub| sub.id != sub_id);
                    if !subs.is_empty() {
                        self.prior_op_subs.insert((serial_id, sub_action), subs);
                    }
                }
            }
            TX_SUB_PREFIX => {
                let hash = TxHash::from_hex(sub_unique_id)?;
                if let Some(mut subs) = self.tx_subs.remove(&(hash.clone(), sub_action)) {
                    subs.retain(|sub| sub.id != sub_id);
                    if !subs.is_empty() {
                        self.tx_subs.insert((hash, sub_action), subs);
                    }
                }
            }
            ACCOUNT_SUB_PREFIX => {
                let account_id: AccountId = sub_unique_id.parse()?;
                if let Some(mut subs) = self.account_subs.remove(&(account_id, sub_action)) {
                    subs.retain(|sub| sub.id != sub_id);
                    if !subs.is_empty() {
                        self.account_subs.insert((account_id, sub_action), subs);
                    }
                }
            }
            _ => return Err(incorrect_id_err()),
        }
        Ok(())
    }

    fn handle_notify_req(&mut self, new_sub: EventNotifierRequest) -> Result<(), failure::Error> {
        match new_sub {
            EventNotifierRequest::Sub(event_sub) => match event_sub {
                EventSubscribeRequest::Transaction {
                    hash,
                    action,
                    subscriber,
                } => self.handle_transaction_sub(hash, action, subscriber),
                EventSubscribeRequest::PriorityOp {
                    serial_id,
                    action,
                    subscriber,
                } => self.handle_priority_op_sub(serial_id, action, subscriber),
                EventSubscribeRequest::Account {
                    address,
                    action,
                    subscriber,
                } => self.handle_account_update_sub(address, action, subscriber),
            }
            .map_err(|e| format_err!("Failed to add sub: {}", e)),
            EventNotifierRequest::Unsub(sub_id) => self
                .handle_unsub(sub_id)
                .map_err(|e| format_err!("Failed to remove sub: {}", e)),
        }
    }

    fn handle_priority_op_sub(
        &mut self,
        serial_id: u64,
        action: ActionType,
        sub: Subscriber<ETHOpInfoResp>,
    ) -> Result<(), failure::Error> {
        let sub_id = SubscriptionId::String(format!(
            "{}/{}/{}/{}",
            ETHOP_SUB_PREFIX,
            serial_id,
            action.to_string(),
            rand::random::<u64>()
        ));

        // Maybe it was executed already
        let storage = self.db_pool.access_storage()?;
        let executed_op = storage.get_executed_priority_op(serial_id as u32)?;
        if let Some(executed_op) = executed_op {
            let block_info = if let Some(block_with_op) =
                storage.get_block(executed_op.block_number as u32)?
            {
                let verified = if let Some(block_verify) = storage.load_stored_op_with_block_number(
                    executed_op.block_number as u32,
                    ActionType::VERIFY,
                ) {
                    block_verify.confirmed
                } else {
                    false
                };

                BlockInfo {
                    block_number: block_with_op.block_number as i64,
                    committed: true,
                    verified,
                }
            } else {
                bail!("Transaction is executed but block is not committed. (bug)");
            };

            match action {
                ActionType::COMMIT => {
                    let sink = sub
                        .assign_id(sub_id)
                        .map_err(|_| format_err!("SubIdAssign"))?;
                    send_once(
                        sink,
                        ETHOpInfoResp {
                            executed: true,
                            block: Some(block_info),
                        },
                    );
                    return Ok(());
                }
                ActionType::VERIFY => {
                    if block_info.verified {
                        let sink = sub
                            .assign_id(sub_id)
                            .map_err(|_| format_err!("SubIdAssign"))?;
                        send_once(
                            sink,
                            ETHOpInfoResp {
                                executed: true,
                                block: Some(block_info),
                            },
                        );
                        return Ok(());
                    }
                }
            }
        }

        let mut subs = self
            .prior_op_subs
            .remove(&(serial_id, action))
            .unwrap_or_default();
        if subs.len() < MAX_LISTENERS_PER_ENTITY {
            let sink = sub
                .assign_id(sub_id.clone())
                .map_err(|_| format_err!("SubIdAssign"))?;
            subs.push(SubscriptionSender { id: sub_id, sink });
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
        let id = SubscriptionId::String(format!(
            "{}/{}/{}/{}",
            TX_SUB_PREFIX,
            hash.to_hex(),
            action.to_string(),
            rand::random::<u64>()
        ));

        // Maybe tx was executed already.
        if let Some(receipt) = self.db_pool.access_storage()?.tx_receipt(hash.as_ref())? {
            let tx_info_resp = TransactionInfoResp {
                executed: true,
                success: Some(receipt.success),
                fail_reason: receipt.fail_reason,
                block: Some(BlockInfo {
                    block_number: receipt.block_number,
                    committed: receipt.success,
                    verified: receipt.verified,
                }),
            };
            match action {
                ActionType::COMMIT => {
                    let sink = sub.assign_id(id).map_err(|_| format_err!("SubIdAssign"))?;
                    send_once(sink, tx_info_resp);
                    return Ok(());
                }
                ActionType::VERIFY => {
                    if receipt.verified {
                        let sink = sub.assign_id(id).map_err(|_| format_err!("SubIdAssign"))?;
                        send_once(sink, tx_info_resp);
                        return Ok(());
                    }
                }
            }
        }

        let mut subs = self
            .tx_subs
            .remove(&(hash.clone(), action))
            .unwrap_or_default();
        if subs.len() < MAX_LISTENERS_PER_ENTITY {
            let sink = sub
                .assign_id(id.clone())
                .map_err(|_| format_err!("SubIdAssign"))?;
            subs.push(SubscriptionSender { id, sink });
            trace!("tx sub added: {}", hash.to_hex());
        }
        self.tx_subs.insert((hash, action), subs);
        Ok(())
    }

    fn handle_account_update_sub(
        &mut self,
        address: AccountAddress,
        action: ActionType,
        sub: Subscriber<ResponseAccountState>,
    ) -> Result<(), failure::Error> {
        let storage = self.db_pool.access_storage()?;
        let account_state = storage.account_state_by_address(&address)?;

        let account_id = if let Some(id) = account_state.committed.as_ref().map(|(id, _)| id) {
            *id
        } else {
            bail!("AccountId is unkwown");
        };

        let sub_id = SubscriptionId::String(format!(
            "{}/{}/{}/{}",
            ACCOUNT_SUB_PREFIX,
            address.to_hex(),
            action.to_string(),
            rand::random::<u64>()
        ));

        let account_state = if let Some(account) = match action {
            ActionType::COMMIT => account_state.committed,
            ActionType::VERIFY => account_state.verified,
        }
        .map(|(_, a)| a)
        {
            let tokens = storage.load_tokens()?;
            ResponseAccountState::try_to_restore(account, &tokens)?
        } else {
            ResponseAccountState::default()
        };

        let mut subs = self
            .account_subs
            .remove(&(account_id, action))
            .unwrap_or_default();
        if subs.len() < MAX_LISTENERS_PER_ENTITY {
            let sink = sub
                .assign_id(sub_id.clone())
                .map_err(|_| format_err!("SubIdAssign"))?;

            send_once(sink.clone(), account_state);
            subs.push(SubscriptionSender { id: sub_id, sink });
        }

        self.account_subs.insert((account_id, action), subs);
        Ok(())
    }

    fn handle_new_block(&mut self, op: Operation) -> Result<(), failure::Error> {
        let storage = self.db_pool.access_storage()?;
        let action = op.action.get_type();

        for tx in op.block.block_transactions {
            match tx {
                ExecutedOperations::Tx(tx) => {
                    let hash = tx.tx.hash();
                    if let Some(subs) = self.tx_subs.remove(&(hash, action)) {
                        let rec = TransactionInfoResp {
                            executed: true,
                            success: Some(tx.success),
                            fail_reason: tx.fail_reason,
                            block: Some(BlockInfo {
                                block_number: op.block.block_number as i64,
                                committed: true,
                                verified: action == ActionType::VERIFY,
                            }),
                        };
                        for sub in subs {
                            send_once(sub.sink, rec.clone());
                        }
                    }
                }
                ExecutedOperations::PriorityOp(prior_op) => {
                    let id = prior_op.priority_op.serial_id;
                    if let Some(subs) = self.prior_op_subs.remove(&(id, action)) {
                        let rec = ETHOpInfoResp {
                            executed: true,
                            block: Some(BlockInfo {
                                block_number: op.block.block_number as i64,
                                committed: true,
                                verified: action == ActionType::VERIFY,
                            }),
                        };
                        for sub in subs {
                            send_once(sub.sink, rec.clone());
                        }
                    }
                }
            }
        }

        let updated_accounts = op.accounts_updated.iter().map(|(id, _)| *id);
        let tokens = storage.load_tokens()?;

        for id in updated_accounts {
            if let Some(subs) = self.account_subs.remove(&(id, action)) {
                let stored_account = match action {
                    ActionType::COMMIT => storage.last_committed_state_for_account(id)?,
                    ActionType::VERIFY => storage.last_verified_state_for_account(id)?,
                };

                let account = if let Some(account) = stored_account {
                    if let Ok(result) = ResponseAccountState::try_to_restore(account, &tokens) {
                        result
                    } else {
                        warn!(
                            "Failed to restore resp account state: id: {}, block: {}",
                            id, op.block.block_number
                        );
                        continue;
                    }
                } else {
                    warn!(
                        "Account is updated but not stored in DB, id: {}, block: {}",
                        id, op.block.block_number
                    );
                    continue;
                };

                for sub in &subs {
                    spawn(sub.sink.notify(Ok(account.clone())).map(drop).map_err(drop));
                }
            }
        }

        Ok(())
    }
}

pub fn start_sub_notifier<BStream, SStream>(
    db_pool: ConnectionPool,
    new_block_stream: BStream,
    subscription_stream: SStream,
    panic_notify: std::sync::mpsc::Sender<bool>,
) where
    BStream: Stream<Item = Operation, Error = ()> + Send + 'static,
    SStream: Stream<Item = EventNotifierRequest, Error = ()> + Send + 'static,
{
    let notifier = OperationNotifier {
        db_pool,
        tx_subs: BTreeMap::new(),
        prior_op_subs: BTreeMap::new(),
        account_subs: BTreeMap::new(),
    };
    let input_stream = new_block_stream
        .map(BlockNotifierInput::NewOperationCommitted)
        .select(subscription_stream.map(BlockNotifierInput::EventNotifyRequest));
    std::thread::Builder::new()
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);

            tokio::run(notifier.run(input_stream));
        })
        .expect("thread start");
}
