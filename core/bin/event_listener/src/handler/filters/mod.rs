use zksync_storage::event::types::ZkSyncEvent;

mod account;
mod block;
mod transaction;

pub use account::AccountFilter;
pub use block::BlockFilter;
pub use transaction::TransactionFilter;

#[derive(Debug, Clone)]
pub enum EventFilter {
    Account(AccountFilter),
    Block(BlockFilter),
    _Transaction(TransactionFilter),
}

impl EventFilter {
    // Should parse json: will be implemented with the transport component. (TODO)
    // pub fn new(...) -> Self {}

    pub fn matches(&self, event: &ZkSyncEvent) -> bool {
        match self {
            EventFilter::Account(account_filter) => account_filter.matches(event),
            EventFilter::Block(block_filter) => block_filter.matches(event),
            EventFilter::_Transaction(tx_filter) => tx_filter.matches(event),
        }
    }
}
