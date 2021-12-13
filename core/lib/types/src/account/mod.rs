// Built-in deps
use std::collections::HashMap;
// External uses
use num::{BigUint, Zero};
use serde::{Deserialize, Serialize};
// Workspace uses
use zksync_crypto::{
    circuit::{
        account::{Balance, CircuitAccount},
        utils::eth_address_to_fr,
    },
    franklin_crypto::bellman::pairing::ff::PrimeField,
    primitives::GetBits,
};
use zksync_utils::BigUintSerdeWrapper;
// Local uses
use super::{AccountId, AccountUpdates, Address, Fr, Nonce, TokenId, NFT};

pub use self::{account_update::AccountUpdate, pubkey_hash::PubKeyHash};

mod account_update;
pub mod error;
mod pubkey_hash;

/// zkSync network account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Account {
    /// Hash of the account public key used to authorize operations for this account.
    /// Once account is created (e.g. by `Transfer` or `Deposit` operation), account owner
    /// has to set its public key hash via `ChangePubKey` transaction, so the server will be
    /// able to verify owner's identity when processing account transactions.
    pub pub_key_hash: PubKeyHash,
    /// Address of the account. Directly corresponds to the L1 address.
    pub address: Address,
    balances: HashMap<TokenId, BigUintSerdeWrapper>,
    /// Current nonce of the account. All the transactions require nonce field to be set in
    /// order to not allow double spend, and the nonce must increment by one after each operation.
    pub nonce: Nonce,
    pub minted_nfts: HashMap<TokenId, NFT>,
}

impl PartialEq for Account {
    fn eq(&self, other: &Account) -> bool {
        // Check for simple fields equality first.
        let basic_equal = self.nonce == other.nonce
            && self.address == other.address
            && self.pub_key_hash == other.pub_key_hash
            && self.minted_nfts == other.minted_nfts;
        if !basic_equal {
            return false;
        }

        // Now we have to compare balances. It's harder, since a zero balance can be represented
        // either as zero balance in hashmap, or as no element in hashmap at all.
        let mut non_zero_balances_self: Vec<_> = self
            .balances
            .iter()
            .filter(|(_token, balance)| !balance.0.is_zero())
            .collect();
        let mut non_zero_balances_other: Vec<_> = other
            .balances
            .iter()
            .filter(|(_token, balance)| !balance.0.is_zero())
            .collect();

        non_zero_balances_self.sort_unstable();
        non_zero_balances_other.sort_unstable();

        non_zero_balances_self == non_zero_balances_other
    }
}

impl From<Account> for CircuitAccount<super::Engine> {
    fn from(acc: Account) -> Self {
        let mut circuit_account = CircuitAccount::default();

        for (i, b) in acc.balances.iter().map(|(id, b)| {
            (
                *id,
                Balance {
                    value: Fr::from_str(&b.0.to_string()).unwrap(),
                },
            )
        }) {
            circuit_account.subtree.insert(*i, b);
        }

        circuit_account.nonce = Fr::from_str(&acc.nonce.to_string()).unwrap();
        circuit_account.pub_key_hash = acc.pub_key_hash.as_fr();
        circuit_account.address = eth_address_to_fr(&acc.address);
        circuit_account
    }
}

impl Default for Account {
    fn default() -> Self {
        Self {
            balances: HashMap::new(),
            nonce: Nonce(0),
            pub_key_hash: PubKeyHash::default(),
            address: Address::zero(),
            minted_nfts: HashMap::new(),
        }
    }
}

impl GetBits for Account {
    fn get_bits_le(&self) -> Vec<bool> {
        CircuitAccount::<super::Engine>::from(self.clone()).get_bits_le()
    }
}

impl Account {
    /// Checks whether this object is an empty default account (equivalent to non-existing account).
    pub fn is_default(&self) -> bool {
        // Checks are sorted so that cheap ones go first.
        // Check for `balances` works so that it returns `true` if `balances` is empty
        // or consists of the 0 balances only.
        self.nonce == Nonce(0)
            && self.address == Address::zero()
            && self.pub_key_hash == PubKeyHash::zero()
            && self.minted_nfts.is_empty()
            && self
                .balances
                .iter()
                .all(|(_token, balance)| balance.0.is_zero())
    }

    /// Creates a new empty account object, and sets its address.
    pub fn default_with_address(address: &Address) -> Account {
        Account {
            address: *address,
            ..Default::default()
        }
    }

    /// Creates a new account object and the list of updates that has to be applied on the state
    /// in order to get this account created within the network.
    pub fn create_account(id: AccountId, address: Address) -> (Account, AccountUpdates) {
        let account = Account::default_with_address(&address);
        let updates = vec![(
            id,
            AccountUpdate::Create {
                address: account.address,
                nonce: account.nonce,
            },
        )];
        (account, updates)
    }

    /// Returns the token balance for the account.
    pub fn get_balance(&self, token: TokenId) -> BigUint {
        self.balances.get(&token).cloned().unwrap_or_default().0
    }

    /// Overrides the token balance value.
    pub fn set_balance(&mut self, token: TokenId, amount: BigUint) {
        self.balances.insert(token, amount.into());
    }

    /// Adds the provided amount to the token balance.
    pub fn add_balance(&mut self, token: TokenId, amount: &BigUint) {
        let mut balance = self.balances.remove(&token).unwrap_or_default();
        balance.0 += amount;
        self.balances.insert(token, balance);
    }

    /// Subtracts the provided amount from the token balance.
    ///
    /// # Panics
    ///
    /// Panics if the amount to subtract is greater than the existing token balance.
    pub fn sub_balance(&mut self, token: TokenId, amount: &BigUint) {
        let mut balance = self.balances.remove(&token).unwrap_or_default();
        balance.0 -= amount;
        self.balances.insert(token, balance);
    }

    /// Given the list of updates to apply, changes the account state.
    pub fn apply_updates(mut account: Option<Self>, updates: &[AccountUpdate]) -> Option<Self> {
        for update in updates {
            account = Account::apply_update(account, update.clone());
        }
        account
    }

    /// Applies an update to the account state.
    pub fn apply_update(account: Option<Self>, update: AccountUpdate) -> Option<Self> {
        match account {
            Some(mut account) => match update {
                AccountUpdate::Delete { .. } => None,
                AccountUpdate::UpdateBalance {
                    balance_update: (token, _, amount),
                    new_nonce,
                    ..
                } => {
                    account.set_balance(token, amount);
                    account.nonce = new_nonce;
                    Some(account)
                }
                AccountUpdate::ChangePubKeyHash {
                    new_pub_key_hash,
                    new_nonce,
                    ..
                } => {
                    account.pub_key_hash = new_pub_key_hash;
                    account.nonce = new_nonce;
                    Some(account)
                }
                AccountUpdate::MintNFT { token, .. } => {
                    account.minted_nfts.insert(token.id, token);
                    Some(account)
                }
                AccountUpdate::RemoveNFT { token, .. } => {
                    account.minted_nfts.remove(&token.id);
                    Some(account)
                }
                _ => {
                    vlog::error!(
                        "Incorrect update received {:?} for account {:?}",
                        update,
                        account
                    );
                    Some(account)
                }
            },
            None => match update {
                AccountUpdate::Create { address, nonce, .. } => Some(Account {
                    address,
                    nonce,
                    ..Default::default()
                }),
                _ => {
                    vlog::error!("Incorrect update received {:?} for empty account", update);
                    None
                }
            },
        }
    }

    /// Returns all the nonzero token balances for the account.
    pub fn get_nonzero_balances(&self) -> HashMap<TokenId, BigUintSerdeWrapper> {
        let mut balances = self.balances.clone();
        balances.retain(|_, v| v.0 != BigUint::zero());
        balances
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{
        helpers::{apply_updates, reverse_updates},
        AccountMap,
    };

    fn sample_nft(token_id: TokenId) -> NFT {
        NFT::new(
            token_id,
            1,
            AccountId(1),
            Default::default(),
            Default::default(),
            None,
            Default::default(),
        )
    }

    fn sample_pubkey_hash(byte: u8) -> PubKeyHash {
        PubKeyHash {
            data: [byte; zksync_crypto::params::FR_ADDRESS_LEN],
        }
    }

    #[test]
    fn is_default_account() {
        let mut empty_account = Account::default();
        assert!(empty_account.is_default());

        empty_account.add_balance(TokenId(0), &0u64.into());
        assert!(empty_account.is_default());
        empty_account.add_balance(TokenId(10), &0u64.into());
        assert!(empty_account.is_default());

        // Change different fields of account and check that it's not "empty" anymore.
        let mut account = empty_account.clone();
        account.pub_key_hash = sample_pubkey_hash(0xFF);
        assert!(!account.is_default(), "Pubkey hash was set");

        let mut account = empty_account.clone();
        account.address = Address::repeat_byte(0xAA);
        assert!(!account.is_default(), "Address was set");

        let mut account = empty_account.clone();
        account.nonce = Nonce(1);
        assert!(!account.is_default(), "Nonce was set");

        let mut account = empty_account.clone();
        account
            .minted_nfts
            .insert(TokenId(1000), sample_nft(TokenId(1000)));
        assert!(!account.is_default(), "Account has minted NFT");

        let mut account = empty_account.clone();
        account.add_balance(TokenId(100), &100u64.into());
        assert!(!account.is_default(), "Account has non-zero balance");
    }

    #[test]
    fn is_deeply_equal_account() {
        // Default account equals default account.
        assert_eq!(Account::default(), Account::default());

        // Empty account with zero balance equals to default account.
        let mut account = Account::default();
        account.add_balance(TokenId(0), &0u64.into());

        assert_eq!(account, Account::default());
        assert_eq!(Account::default(), account);

        // Empty account with zero balance equals to account with another zero token balance.
        let mut account_1 = Account::default();
        account_1.add_balance(TokenId(0), &0u64.into());
        let mut account_2 = Account::default();
        account_2.add_balance(TokenId(42), &0u64.into());
        assert_eq!(account_1, account_2);
        assert_eq!(account_2, account_1);

        // Accounts with different nonces are different.
        let account_1 = Account {
            nonce: Nonce(2),
            ..Account::default()
        };
        let account_2 = Account {
            nonce: Nonce(3),
            ..Account::default()
        };
        assert_ne!(account_1, account_2);
        assert_ne!(account_2, account_1);

        // Accounts with different addresses are different.
        let account_1 = Account {
            address: Address::repeat_byte(0xAA),
            ..Account::default()
        };
        let account_2 = Account {
            address: Address::repeat_byte(0xBB),
            ..Account::default()
        };
        assert_ne!(account_1, account_2);
        assert_ne!(account_2, account_1);

        // Accounts with different pubkey hashes are different.
        let account_1 = Account {
            pub_key_hash: sample_pubkey_hash(0xAA),
            ..Account::default()
        };
        let account_2 = Account {
            pub_key_hash: sample_pubkey_hash(0xBB),
            ..Account::default()
        };
        assert_ne!(account_1, account_2);
        assert_ne!(account_2, account_1);

        // Accounts with different NFTs are different.
        let mut account_1 = Account::default();
        account_1
            .minted_nfts
            .insert(TokenId(1000), sample_nft(TokenId(1000)));
        let account_2 = Account::default();
        assert_ne!(account_1, account_2);
        assert_ne!(account_2, account_1);

        // Accounts with different balances are different.
        let mut account_1 = Account::default();
        account_1
            .minted_nfts
            .insert(TokenId(1000), sample_nft(TokenId(1000)));
        let account_2 = Account::default();
        assert_ne!(account_1, account_2);
        assert_ne!(account_2, account_1);
    }

    #[test]
    fn test_default_account() {
        let a = Account::default();
        a.get_bits_le();
    }

    #[test]
    fn test_account_update() {
        let create = AccountUpdate::Create {
            address: Address::default(),
            nonce: Nonce(1),
        };

        let bal_update = AccountUpdate::UpdateBalance {
            old_nonce: Nonce(1),
            new_nonce: Nonce(2),
            balance_update: (TokenId(0), 0u32.into(), 5u32.into()),
        };

        let delete = AccountUpdate::Delete {
            address: Address::default(),
            nonce: Nonce(2),
        };

        {
            {
                let created_account = Account {
                    nonce: Nonce(1),
                    ..Default::default()
                };
                assert_eq!(
                    Account::apply_update(None, create.clone())
                        .unwrap()
                        .get_bits_le(),
                    created_account.get_bits_le()
                );
            }

            assert!(Account::apply_update(None, bal_update.clone()).is_none());
            assert!(Account::apply_update(None, delete.clone()).is_none());
        }
        {
            assert_eq!(
                Account::apply_update(Some(Account::default()), create)
                    .unwrap()
                    .get_bits_le(),
                Account::default().get_bits_le()
            );
            {
                let mut updated_account = Account {
                    nonce: Nonce(2),
                    ..Default::default()
                };
                updated_account.set_balance(TokenId(0), 5u32.into());
                assert_eq!(
                    Account::apply_update(Some(Account::default()), bal_update)
                        .unwrap()
                        .get_bits_le(),
                    updated_account.get_bits_le()
                );
            }
            assert!(Account::apply_update(Some(Account::default()), delete).is_none());
        }
    }

    #[test]
    fn test_account_updates() {
        // Create two accounts: 0, 1
        // In updates -> delete 0, update balance of 1, create account 2
        // Reverse updates

        let account_map_initial = {
            let mut map = AccountMap::default();
            let account_0 = Account {
                nonce: Nonce(8),
                ..Default::default()
            };
            let account_1 = Account {
                nonce: Nonce(16),
                ..Default::default()
            };
            map.insert(AccountId(0), account_0);
            map.insert(AccountId(1), account_1);
            map
        };

        let account_map_updated_expected = {
            let mut map = AccountMap::default();
            let mut account_1 = Account {
                nonce: Nonce(17),
                ..Default::default()
            };
            account_1.set_balance(TokenId(0), 256u32.into());
            map.insert(AccountId(1), account_1);
            let account_2 = Account {
                nonce: Nonce(36),
                ..Default::default()
            };
            map.insert(AccountId(2), account_2);
            map
        };

        let updates = vec![
            (
                AccountId(0),
                AccountUpdate::Delete {
                    address: Address::default(),
                    nonce: Nonce(8),
                },
            ),
            (
                AccountId(1),
                AccountUpdate::UpdateBalance {
                    old_nonce: Nonce(16),
                    new_nonce: Nonce(17),
                    balance_update: (TokenId(0), 0u32.into(), 256u32.into()),
                },
            ),
            (
                AccountId(2),
                AccountUpdate::Create {
                    address: Address::default(),
                    nonce: Nonce(36),
                },
            ),
        ];

        let account_map_updated = {
            let mut map = account_map_initial.clone();
            apply_updates(&mut map, updates.clone());
            map
        };

        assert_eq!(account_map_updated, account_map_updated_expected);

        let account_map_updated_back = {
            let mut map = account_map_updated;
            let mut reversed = updates;
            reverse_updates(&mut reversed);
            apply_updates(&mut map, reversed);
            map
        };

        assert_eq!(account_map_updated_back, account_map_initial);
    }
}
