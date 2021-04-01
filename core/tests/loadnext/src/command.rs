use zksync_types::{AccountId, Address, TokenId, U256};

#[derive(Debug, Copy, Clone)]
pub enum CommandType {
    Deposit,
    Withdraw,
    Transfer,
    FullExit,
}

#[derive(Debug, Copy, Clone)]
pub enum Target {
    Address(Address),
    AccountId(AccountId),
}

#[derive(Debug, Copy, Clone)]
pub struct Command {
    pub command_type: CommandType,
    pub to: Target,
    pub token: TokenId,
    pub amount: U256,
}
