pub mod account;
pub mod block;
pub mod circuit;
pub mod params;
pub mod tx;

pub use web3::types::{H256, U128, U256};

// use merkle_tree::{PedersenHasher, SparseMerkleTree};
use pairing::bn256;
use sapling_crypto::eddsa;

pub use crate::plasma::account::{Account, AccountUpdate};
pub use crate::plasma::tx::{DepositTx, ExitTx, TransferTx, TxSignature};

pub type Engine = bn256::Bn256;
pub type Fr = bn256::Fr;

pub type AccountMap = fnv::FnvHashMap<u32, Account>;
pub type AccountUpdateMap = Vec<AccountUpdate>;

pub fn apply_updates(accounts: &mut AccountMap, updates: AccountUpdateMap) {
    for update in updates.into_iter() {
        let id = update.get_account_id();
        let updated_account = Account::apply_update(accounts.remove(&id), update);
        if let Some(account) = updated_account {
            accounts.insert(id, account);
        }
    }
}

pub type PublicKey = eddsa::PublicKey<Engine>;
pub type PrivateKey = eddsa::PrivateKey<Engine>;

pub type BatchNumber = u32;
pub type BlockNumber = u32;
pub type AccountId = u32;
pub type Nonce = u32;

#[derive(Debug)]
pub enum TransferApplicationError {
    Unknown,
    InsufficientBalance,
    NonceIsTooLow,
    NonceIsTooHigh,
    UnknownSigner,
    InvalidSigner,
    ExpiredTransaction,
    InvalidTransaction(String),
}
