use crate::api_server::rpc_server::types::{
    BlockInfo, ETHOpInfoResp, ResponseAccountState, TransactionInfoResp,
};
use jsonrpc_pubsub::{typed::Subscriber, SubscriptionId};
use zksync_storage::ConnectionPool;
use zksync_types::tx::TxHash;
use zksync_types::BlockNumber;
use zksync_types::{block::ExecutedOperations, AccountId, ActionType, Address, Operation};

use super::{
    state::NotifierState, sub_store::SubStorage, EventNotifierRequest, EventSubscribeRequest,
    ExecutedOps,
};

pub struct OperationNotifier {
    state: NotifierState,

    tx_subs: SubStorage<TxHash, TransactionInfoResp>,
    prior_op_subs: SubStorage<u64, ETHOpInfoResp>,
    account_subs: SubStorage<AccountId, ResponseAccountState>,
}

impl OperationNotifier {
    pub fn new(cache_capacity: usize, db_pool: ConnectionPool) -> Self {
        Self {
            state: NotifierState::new(cache_capacity, db_pool),
            tx_subs: SubStorage::new(),
            prior_op_subs: SubStorage::new(),
            account_subs: SubStorage::new(),
        }
    }

    /// Handles incoming subscription/unsubscription request.
    pub async fn handle_notify_req(
        &mut self,
        new_sub: EventNotifierRequest,
    ) -> Result<(), anyhow::Error> {
        match new_sub {
            EventNotifierRequest::Sub(event_sub) => match event_sub {
                EventSubscribeRequest::Transaction {
                    hash,
                    action,
                    subscriber,
                } => self.add_transaction_sub(hash, action, subscriber).await,
                EventSubscribeRequest::PriorityOp {
                    serial_id,
                    action,
                    subscriber,
                } => {
                    self.add_priority_op_sub(serial_id, action, subscriber)
                        .await
                }
                EventSubscribeRequest::Account {
                    address,
                    action,
                    subscriber,
                } => {
                    self.add_account_update_sub(address, action, subscriber)
                        .await
                }
            }
            .map_err(|e| anyhow::format_err!("Failed to add sub: {}", e)),
            EventNotifierRequest::Unsub(sub_id) => self
                .handle_unsub(sub_id)
                .map_err(|e| anyhow::format_err!("Failed to remove sub: {}", e)),
        }
    }

    /// Processes new block action (commit or verify), notifying the subscribers.
    pub async fn handle_new_block(&mut self, op: Operation) -> Result<(), anyhow::Error> {
        let action = op.action.get_type();

        self.handle_executed_operations(
            op.block.block_transactions.clone(),
            action,
            op.block.block_number,
        )?;

        let updated_accounts: Vec<AccountId> = op
            .block
            .block_transactions
            .iter()
            .map(|exec_op| exec_op.get_updated_account_ids())
            .flatten()
            .collect();

        for id in updated_accounts {
            if self.account_subs.subscriber_exists(id, action) {
                let account_state = match self.state.get_account_state(id, action).await? {
                    Some(account_state) => account_state,
                    None => {
                        log::warn!(
                            "Account is updated but not stored in DB, id: {}, block: {:#?}",
                            id,
                            op.block
                        );
                        continue;
                    }
                };

                self.account_subs.notify(id, action, account_state);
            }
        }

        Ok(())
    }

    /// Processes new executed operations, notifying the corresponding subscribers.
    fn handle_executed_operations(
        &mut self,
        ops: Vec<ExecutedOperations>,
        action: ActionType,
        block_number: BlockNumber,
    ) -> Result<(), anyhow::Error> {
        for tx in ops {
            match tx {
                ExecutedOperations::Tx(tx) => {
                    let hash = tx.signed_tx.hash();
                    let resp = TransactionInfoResp {
                        executed: true,
                        success: Some(tx.success),
                        fail_reason: tx.fail_reason,
                        block: Some(BlockInfo {
                            block_number: i64::from(block_number),
                            committed: true,
                            verified: action == ActionType::VERIFY,
                        }),
                    };
                    self.tx_subs.notify(hash, action, resp);
                }
                ExecutedOperations::PriorityOp(prior_op) => {
                    let id = prior_op.priority_op.serial_id;
                    let resp = ETHOpInfoResp {
                        executed: true,
                        block: Some(BlockInfo {
                            block_number: i64::from(block_number),
                            committed: true,
                            verified: action == ActionType::VERIFY,
                        }),
                    };
                    self.prior_op_subs.notify(id, action, resp);
                }
            }
        }
        Ok(())
    }

    /// More convenient alias for `handle_executed_operations`.
    pub fn handle_new_executed_batch(
        &mut self,
        exec_batch: ExecutedOps,
    ) -> Result<(), anyhow::Error> {
        self.handle_executed_operations(
            exec_batch.operations,
            ActionType::COMMIT,
            exec_batch.block_number,
        )
    }

    /// Removes provided subscription from the list.
    fn handle_unsub(&mut self, sub_id: SubscriptionId) -> Result<(), anyhow::Error> {
        self.prior_op_subs.remove(sub_id.clone())?;
        self.tx_subs.remove(sub_id.clone())?;
        self.account_subs.remove(sub_id)?;
        Ok(())
    }

    /// Add priority operation subscription.
    async fn add_priority_op_sub(
        &mut self,
        serial_id: u64,
        action: ActionType,
        sub: Subscriber<ETHOpInfoResp>,
    ) -> Result<(), anyhow::Error> {
        let sub_id = self.prior_op_subs.generate_sub_id(serial_id, action);

        let executed_op = self
            .state
            .get_executed_priority_operation(serial_id as u32)
            .await?;
        if let Some(executed_op) = executed_op {
            // There may be no block, if transaction was executed in the pending block only.
            if let Some(block_info) = self
                .state
                .get_block_info(executed_op.block_number as u32)
                .await?
            {
                match action {
                    ActionType::COMMIT => {
                        let resp = ETHOpInfoResp {
                            executed: true,
                            block: Some(block_info),
                        };
                        self.prior_op_subs.respond_once(sub_id, sub, resp)?;
                        return Ok(());
                    }
                    ActionType::VERIFY => {
                        if block_info.verified {
                            let resp = ETHOpInfoResp {
                                executed: true,
                                block: Some(block_info),
                            };
                            self.prior_op_subs.respond_once(sub_id, sub, resp)?;
                            return Ok(());
                        }
                    }
                }
            }
        }

        self.prior_op_subs
            .insert_new(sub_id, sub, serial_id, action)?;
        Ok(())
    }

    /// Add transactions subscription.
    async fn add_transaction_sub(
        &mut self,
        hash: TxHash,
        action: ActionType,
        sub: Subscriber<TransactionInfoResp>,
    ) -> Result<(), anyhow::Error> {
        let sub_id = self.tx_subs.generate_sub_id(hash, action);

        let tx_receipt = self.state.get_tx_receipt(&hash).await?;

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
                    self.tx_subs.respond_once(sub_id, sub, tx_info_resp)?;
                    return Ok(());
                }
                ActionType::VERIFY => {
                    if receipt.verified {
                        self.tx_subs.respond_once(sub_id, sub, tx_info_resp)?;
                        return Ok(());
                    }
                }
            }
        }

        self.tx_subs.insert_new(sub_id, sub, hash, action)?;
        Ok(())
    }

    /// Add account info subscription.
    async fn add_account_update_sub(
        &mut self,
        address: Address,
        action: ActionType,
        sub: Subscriber<ResponseAccountState>,
    ) -> Result<(), anyhow::Error> {
        let (account_id, _account_state) = self.state.get_account_info(address, action).await?;

        let sub_id = self.account_subs.generate_sub_id(account_id, action);

        self.account_subs
            .insert_new(sub_id, sub, account_id, action)?;
        Ok(())
    }
}
