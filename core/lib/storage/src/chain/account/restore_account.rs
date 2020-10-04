// Built-in deps
// External imports
use num::bigint::ToBigInt;
use zksync_basic_types::Address;
// Workspace imports
use zksync_types::PubKeyHash;
use zksync_types::{Account, AccountId, TokenId};
// Local imports
use super::records::*;

pub(crate) fn restore_account(
    stored_account: &StorageAccount,
    stored_balances: Vec<StorageBalance>,
) -> (AccountId, Account) {
    let mut account = Account::default();
    for b in stored_balances.into_iter() {
        assert_eq!(b.account_id, stored_account.id);
        let balance_bigint = b.balance.to_bigint().unwrap();
        let balance = balance_bigint.to_biguint().unwrap();
        account.set_balance(b.coin_id as TokenId, balance);
    }
    account.nonce = stored_account.nonce as u32;
    account.address = Address::from_slice(&stored_account.address);
    account.pub_key_hash = PubKeyHash::from_bytes(&stored_account.pubkey_hash)
        .expect("db stored pubkey hash deserialize");
    (stored_account.id as u32, account)
}
