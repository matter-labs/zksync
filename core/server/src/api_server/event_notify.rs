use super::rpc_server::{ETHOpInfoResp, TransactionInfoResp};
use crate::api_server::rpc_server::{BlockInfo, ResponseAccountState};
use crate::state_keeper::{ExecutedOpId, ExecutedOpsNotify, StateKeeperRequest};
use failure::{bail, format_err};
use futures::task::LocalSpawnExt;
use futures::{
    channel::{mpsc, oneshot},
    compat::Future01CompatExt,
    executor, select,
    stream::StreamExt,
    FutureExt, SinkExt,
};
use jsonrpc_pubsub::{
    typed::{Sink, Subscriber},
    SubscriptionId,
};
use lru_cache::LruCache;
use models::config_options::ThreadPanicNotify;
use models::node::tx::TxHash;
use models::node::BlockNumber;
use models::{node::block::ExecutedOperations, node::AccountId, ActionType, Operation};
use std::collections::BTreeMap;
use std::str::FromStr;
use storage::chain::operations::records::StoredExecutedPriorityOperation;
use storage::chain::operations_ext::records::TxReceiptResponse;
use storage::ConnectionPool;
use web3::types::Address;

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
        address: Address,
        action: ActionType,
        subscriber: Subscriber<ResponseAccountState>,
    },
}

pub enum EventNotifierRequest {
    Sub(EventSubscribeRequest),
    Unsub(SubscriptionId),
}

struct SubscriptionSender<T> {
    id: SubscriptionId,
    sink: Sink<T>,
}

struct OperationNotifier {
    cache_of_executed_priority_operations: LruCache<u32, StoredExecutedPriorityOperation>,
    cache_of_transaction_receipts: LruCache<Vec<u8>, TxReceiptResponse>,
    cache_of_blocks_info: LruCache<BlockNumber, BlockInfo>,
    db_pool: ConnectionPool,
    state_keeper_requests: mpsc::Sender<StateKeeperRequest>,
    tx_subs: BTreeMap<(TxHash, ActionType), Vec<SubscriptionSender<TransactionInfoResp>>>,
    prior_op_subs: BTreeMap<(u64, ActionType), Vec<SubscriptionSender<ETHOpInfoResp>>>,
    account_subs: BTreeMap<(AccountId, ActionType), Vec<SubscriptionSender<ResponseAccountState>>>,

    spawner: executor::LocalSpawner,
}

impl OperationNotifier {
    fn send_once<T: serde::Serialize>(&self, sink: &Sink<T>, val: T) {
        self.spawner
            .spawn_local(sink.notify(Ok(val)).compat().map(drop))
            .expect("future local_spawn");
    }

    async fn check_op_executed_current_block(
        &self,
        op_id: ExecutedOpId,
    ) -> Result<Option<(BlockNumber, bool)>, failure::Error> {
        let response = oneshot::channel();
        self.state_keeper_requests
            .clone()
            .send(StateKeeperRequest::GetExecutedInPendingBlock(
                op_id, response.0,
            ))
            .await?;
        let state_keeper_resp = response.1.await;
        Ok(state_keeper_resp?)
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
                let hash = TxHash::from_str(sub_unique_id)?;
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

    async fn handle_notify_req(
        &mut self,
        new_sub: EventNotifierRequest,
    ) -> Result<(), failure::Error> {
        match new_sub {
            EventNotifierRequest::Sub(event_sub) => match event_sub {
                EventSubscribeRequest::Transaction {
                    hash,
                    action,
                    subscriber,
                } => self.handle_transaction_sub(hash, action, subscriber).await,
                EventSubscribeRequest::PriorityOp {
                    serial_id,
                    action,
                    subscriber,
                } => {
                    self.handle_priority_op_sub(serial_id, action, subscriber)
                        .await
                }
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

    fn get_executed_priority_operation(
        &mut self,
        serial_id: u32,
    ) -> Result<Option<StoredExecutedPriorityOperation>, failure::Error> {
        let res = if let Some(executed_op) = self
            .cache_of_executed_priority_operations
            .get_mut(&serial_id)
        {
            Some(executed_op.clone())
        } else {
            let storage = self.db_pool.access_storage_fragile()?;
            let executed_op = storage
                .chain()
                .operations_schema()
                .get_executed_priority_operation(serial_id)?;

            if let Some(executed_op) = executed_op.clone() {
                self.cache_of_executed_priority_operations
                    .insert(serial_id, executed_op);
            }

            executed_op
        };
        Ok(res)
    }

    fn get_block_info(&mut self, block_number: u32) -> Result<BlockInfo, failure::Error> {
        let res = if let Some(block_info) = self.cache_of_blocks_info.get_mut(&block_number) {
            block_info.clone()
        } else {
            let storage = self.db_pool.access_storage_fragile()?;
            let block_info = if let Some(block_with_op) =
                storage.chain().block_schema().get_block(block_number)?
            {
                let verified = if let Some(block_verify) = storage
                    .chain()
                    .operations_schema()
                    .get_operation(block_number, ActionType::VERIFY)
                {
                    block_verify.confirmed
                } else {
                    false
                };

                BlockInfo {
                    block_number: i64::from(block_with_op.block_number),
                    committed: true,
                    verified,
                }
            } else {
                bail!("Transaction is executed but block is not committed. (bug)");
            };

            // Unverified blocks can still change, so we can't cache them.
            if block_info.verified {
                self.cache_of_blocks_info
                    .insert(block_number, block_info.clone());
            }

            block_info
        };
        Ok(res)
    }

    async fn handle_priority_op_sub(
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
            crypto_exports::rand::random::<u64>()
        ));

        // Maybe it was executed already
        if action == ActionType::COMMIT {
            if let Some((block_number, _)) = self
                .check_op_executed_current_block(ExecutedOpId::PriorityOp(serial_id))
                .await?
            {
                let sink = sub
                    .assign_id(sub_id)
                    .map_err(|_| format_err!("SubIdAssign"))?;
                self.send_once(
                    &sink,
                    ETHOpInfoResp {
                        executed: true,
                        block: Some(BlockInfo {
                            block_number: i64::from(block_number),
                            committed: true,
                            verified: false,
                        }),
                    },
                );
                return Ok(());
            }
        }

        let executed_op = self.get_executed_priority_operation(serial_id as u32)?;
        if let Some(executed_op) = executed_op {
            let block_info = self.get_block_info(executed_op.block_number as u32)?;

            match action {
                ActionType::COMMIT => {
                    let sink = sub
                        .assign_id(sub_id)
                        .map_err(|_| format_err!("SubIdAssign"))?;
                    self.send_once(
                        &sink,
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
                        self.send_once(
                            &sink,
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

    fn get_tx_receipt(
        &mut self,
        hash: &TxHash,
    ) -> Result<Option<TxReceiptResponse>, failure::Error> {
        let res = if let Some(tx_receipt) = self
            .cache_of_transaction_receipts
            .get_mut(&hash.as_ref().to_vec())
        {
            Some(tx_receipt.clone())
        } else {
            let storage = self.db_pool.access_storage_fragile()?;
            let tx_receipt = storage
                .chain()
                .operations_ext_schema()
                .tx_receipt(hash.as_ref())?;

            if let Some(tx_receipt) = tx_receipt.clone() {
                if tx_receipt.verified {
                    self.cache_of_transaction_receipts
                        .insert(hash.as_ref().to_vec(), tx_receipt);
                }
            }

            tx_receipt
        };
        Ok(res)
    }

    async fn handle_transaction_sub(
        &mut self,
        hash: TxHash,
        action: ActionType,
        sub: Subscriber<TransactionInfoResp>,
    ) -> Result<(), failure::Error> {
        let id = SubscriptionId::String(format!(
            "{}/{}/{}/{}",
            TX_SUB_PREFIX,
            hash.to_string(),
            action.to_string(),
            crypto_exports::rand::random::<u64>()
        ));

        // Maybe tx was executed already.
        if action == ActionType::COMMIT {
            if let Some((block_number, success)) = self
                .check_op_executed_current_block(ExecutedOpId::Transaction(hash.clone()))
                .await?
            {
                let sink = sub.assign_id(id).map_err(|_| format_err!("SubIdAssign"))?;
                self.send_once(
                    &sink,
                    TransactionInfoResp {
                        executed: true,
                        success: Some(success),
                        fail_reason: None,
                        block: Some(BlockInfo {
                            block_number: i64::from(block_number),
                            committed: true,
                            verified: false,
                        }),
                    },
                );
                return Ok(());
            }
        }

        let tx_receipt = self.get_tx_receipt(&hash)?;

        if let Some(receipt) = tx_receipt {
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
                    self.send_once(&sink, tx_info_resp);
                    return Ok(());
                }
                ActionType::VERIFY => {
                    if receipt.verified {
                        let sink = sub.assign_id(id).map_err(|_| format_err!("SubIdAssign"))?;
                        self.send_once(&sink, tx_info_resp);
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
            trace!("tx sub added: {}", hash.to_string());
        }
        self.tx_subs.insert((hash, action), subs);
        Ok(())
    }

    fn handle_account_update_sub(
        &mut self,
        address: Address,
        action: ActionType,
        sub: Subscriber<ResponseAccountState>,
    ) -> Result<(), failure::Error> {
        let storage = self.db_pool.access_storage_fragile()?;
        let account_state = storage
            .chain()
            .account_schema()
            .account_state_by_address(&address)?;

        let account_id = if let Some(id) = account_state.committed.as_ref().map(|(id, _)| id) {
            *id
        } else {
            bail!("AccountId is unkwown");
        };

        let sub_id = SubscriptionId::String(format!(
            "{}/{:x}/{}/{}",
            ACCOUNT_SUB_PREFIX,
            address,
            action.to_string(),
            crypto_exports::rand::random::<u64>()
        ));

        let account_state = if let Some(account) = match action {
            ActionType::COMMIT => account_state.committed,
            ActionType::VERIFY => account_state.verified,
        }
        .map(|(_, a)| a)
        {
            let tokens = storage.tokens_schema().load_tokens()?;
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

            self.send_once(&sink, account_state);
            subs.push(SubscriptionSender { id: sub_id, sink });
        }

        self.account_subs.insert((account_id, action), subs);
        Ok(())
    }

    fn handle_executed_operations(
        &mut self,
        ops: Vec<ExecutedOperations>,
        action: ActionType,
        block_number: BlockNumber,
    ) -> Result<(), failure::Error> {
        for tx in ops {
            match tx {
                ExecutedOperations::Tx(tx) => {
                    let hash = tx.tx.hash();
                    if let Some(subs) = self.tx_subs.remove(&(hash, action)) {
                        let rec = TransactionInfoResp {
                            executed: true,
                            success: Some(tx.success),
                            fail_reason: tx.fail_reason,
                            block: Some(BlockInfo {
                                block_number: i64::from(block_number),
                                committed: true,
                                verified: action == ActionType::VERIFY,
                            }),
                        };
                        for sub in subs {
                            self.send_once(&sub.sink, rec.clone());
                        }
                    }
                }
                ExecutedOperations::PriorityOp(prior_op) => {
                    let id = prior_op.priority_op.serial_id;
                    if let Some(subs) = self.prior_op_subs.remove(&(id, action)) {
                        let rec = ETHOpInfoResp {
                            executed: true,
                            block: Some(BlockInfo {
                                block_number: i64::from(block_number),
                                committed: true,
                                verified: action == ActionType::VERIFY,
                            }),
                        };
                        for sub in subs {
                            self.send_once(&sub.sink, rec.clone());
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn handle_new_executed_batch(
        &mut self,
        exec_batch: ExecutedOpsNotify,
    ) -> Result<(), failure::Error> {
        self.handle_executed_operations(
            exec_batch.operations,
            ActionType::COMMIT,
            exec_batch.block_number,
        )
    }

    fn handle_new_block(&mut self, op: Operation) -> Result<(), failure::Error> {
        let storage = self.db_pool.access_storage_fragile()?;
        let action = op.action.get_type();

        self.handle_executed_operations(
            op.block.block_transactions,
            action,
            op.block.block_number,
        )?;

        let updated_accounts = op.accounts_updated.iter().map(|(id, _)| *id);
        let tokens = storage.tokens_schema().load_tokens()?;

        for id in updated_accounts {
            if let Some(subs) = self.account_subs.remove(&(id, action)) {
                let stored_account = match action {
                    ActionType::COMMIT => storage
                        .chain()
                        .account_schema()
                        .last_committed_state_for_account(id)?,
                    ActionType::VERIFY => storage
                        .chain()
                        .account_schema()
                        .last_verified_state_for_account(id)?,
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
                    self.send_once(&sub.sink, account.clone());
                }
            }
        }

        Ok(())
    }
}

pub fn start_sub_notifier(
    db_pool: ConnectionPool,
    mut new_block_stream: mpsc::Receiver<Operation>,
    mut subscription_stream: mpsc::Receiver<EventNotifierRequest>,
    mut executed_tx_stream: mpsc::Receiver<ExecutedOpsNotify>,
    state_keeper_requests: mpsc::Sender<StateKeeperRequest>,
    panic_notify: mpsc::Sender<bool>,
    each_cache_size: usize,
) {
    std::thread::Builder::new()
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);

            let mut local_pool = executor::LocalPool::new();

            let mut notifier = OperationNotifier {
                cache_of_executed_priority_operations: LruCache::new(each_cache_size),
                cache_of_transaction_receipts: LruCache::new(each_cache_size),
                cache_of_blocks_info: LruCache::new(each_cache_size),
                db_pool,
                state_keeper_requests,
                tx_subs: BTreeMap::new(),
                prior_op_subs: BTreeMap::new(),
                account_subs: BTreeMap::new(),
                spawner: local_pool.spawner(),
            };

            let sub_notifier_task = async move {
                loop {
                    select! {
                        new_block = new_block_stream.next() => {
                            if let Some(new_block) = new_block {
                                notifier.handle_new_block(new_block)
                                    .map_err(|e| warn!("Failed to handle new block: {}",e))
                                    .unwrap_or_default();
                            }
                        },
                        new_exec_batch = executed_tx_stream.next() => {
                            if let Some(new_exec_batch) = new_exec_batch {
                                notifier.handle_new_executed_batch(new_exec_batch)
                                    .map_err(|e| warn!("Failed to handle new exec batch: {}",e))
                                    .unwrap_or_default();
                            }
                        },
                        new_sub = subscription_stream.next() => {
                            if let Some(new_sub) = new_sub {
                                notifier.handle_notify_req(new_sub).await
                                    .map_err(|e| warn!("Failed to handle notify request: {}",e))
                                    .unwrap_or_default();
                            }
                        },
                        complete => break,
                    }
                }
            };

            local_pool
                .spawner()
                .spawn_local(sub_notifier_task)
                .expect("sub notify future spawn");
            local_pool.run();
        })
        .expect("thread start");
}
