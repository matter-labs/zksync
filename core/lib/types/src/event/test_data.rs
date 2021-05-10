// Built-in uses
// External uses
use bigdecimal::BigDecimal;
use chrono::Utc;
use once_cell::sync::OnceCell;
// Workspace uses
// Local uses
use super::{account::*, block::*, transaction::*, EventData, EventId, ZkSyncEvent};
use crate::{AccountId, BlockNumber, Nonce, TokenId};

/// Constructs default values for `BlockDetails` struct. Since block events
/// can only be filtered by status, these fields are not used.
fn get_block_details() -> BlockDetails {
    BlockDetails {
        block_number: BlockNumber(0),
        new_state_root: Vec::new(),
        block_size: 0,
        commit_tx_hash: None,
        verify_tx_hash: None,
        committed_at: Utc::now(),
        verified_at: None,
    }
}

/// Construct block event with the given block status.
pub fn get_block_event(block_status: BlockStatus) -> ZkSyncEvent {
    let block_details = get_block_details();
    let block_event = BlockEvent {
        status: block_status,
        block_details,
    };
    ZkSyncEvent {
        id: EventId(0),
        block_number: BlockNumber(0),
        data: EventData::Block(block_event),
    }
}

/// Construct account event with the given account id, token and status.
pub fn get_account_event(
    account_id: AccountId,
    token_id: Option<TokenId>,
    status: AccountStateChangeStatus,
) -> ZkSyncEvent {
    let (update_type, new_balance) = if token_id.is_some() {
        (
            AccountStateChangeType::UpdateBalance,
            Some(BigDecimal::from(100)),
        )
    } else {
        (AccountStateChangeType::Create, None)
    };
    let update_details = AccountUpdateDetails {
        account_id,
        nonce: Nonce(0),
        new_pub_key_hash: None,
        token_id,
        new_balance,
    };
    let account_update = AccountEvent {
        update_type,
        status,
        update_details,
    };
    ZkSyncEvent {
        id: EventId(0),
        block_number: BlockNumber(0),
        data: EventData::Account(account_update),
    }
}

/// Construct transaction event with the given type, account id, token and
/// status.
pub fn get_transaction_event(
    tx_type: TransactionType,
    account_id: AccountId,
    token_id: TokenId,
    status: TransactionStatus,
) -> ZkSyncEvent {
    // Initialize the cell to prevent panic when deserializing
    // empty `tx` json.
    let tx_event = TransactionEvent {
        tx_hash: String::new(),
        account_id,
        token_id,
        block_number: BlockNumber(0),
        tx: Default::default(),
        status,
        fail_reason: None,
        created_at: Utc::now(),
        tx_type: OnceCell::from(tx_type),
    };
    ZkSyncEvent {
        id: EventId(0),
        block_number: BlockNumber(0),
        data: EventData::Transaction(tx_event),
    }
}
