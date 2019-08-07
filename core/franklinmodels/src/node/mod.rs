use super::merkle_tree::{PedersenHasher, SparseMerkleTree};
use bigdecimal::BigDecimal;
use franklin_crypto::eddsa;
use pairing::bn256;

pub mod account;
pub mod block;
pub mod config;
pub mod operations;
pub mod tx;

pub use web3::types::{H256, U128, U256};

pub use self::account::{Account, AccountAddress, AccountUpdate};
pub use self::operations::{DepositOp, FranklinOp, PartialExitOp, TransferOp, TransferToNewOp};

pub type Engine = bn256::Bn256;
pub type Fr = bn256::Fr;

pub type AccountMap = fnv::FnvHashMap<u32, Account>;
pub type AccountUpdates = Vec<(u32, AccountUpdate)>;
pub type AccountTree = SparseMerkleTree<Account, Fr, PedersenHasher<Engine>>;

pub fn apply_updates(accounts: &mut AccountMap, updates: AccountUpdates) {
    for (id, update) in updates.into_iter() {
        let updated_account = Account::apply_update(accounts.remove(&id), update);
        if let Some(account) = updated_account {
            accounts.insert(id, account);
        }
    }
}

pub fn reverse_updates(updates: &mut AccountUpdates) {
    updates.reverse();
    for (_, acc_upd) in updates.iter_mut() {
        *acc_upd = acc_upd.reversed_update();
    }
}

pub type TokenId = u16;

/// 3 bytes used.
pub type AccountId = u32;
pub type BlockNumber = u32;
pub type Nonce = u32;
/// 3 bytes used.
pub type TokenAmount = u32;
pub type FeeAmount = u8;
