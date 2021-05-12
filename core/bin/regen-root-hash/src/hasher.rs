use zksync_crypto::{
    circuit::account::{Balance, CircuitBalanceTree},
    merkle_tree::RescueHasher,
    merkle_tree::SparseMerkleTree,
    primitives::GetBits,
    Engine, Fr,
};

use crate::account::CircuitAccountWrapper;
use once_cell::sync::Lazy;
use zksync_types::Account;

pub static BALANCE_TREE_32: Lazy<CircuitBalanceTree> = Lazy::new(|| SparseMerkleTree::new(32));
pub static BALANCE_TREE_11: Lazy<CircuitBalanceTree> = Lazy::new(|| SparseMerkleTree::new(11));

pub const NUMBER_OF_OLD_TOKENS: u32 = 2u32.pow(11);

pub type CustomMerkleTree<T> = SparseMerkleTree<T, Fr, RescueHasher<Engine>>;

pub fn get_state<T: CircuitAccountWrapper>(accounts: &[(i64, Account)]) -> CustomMerkleTree<T> {
    let mut account_state_tree: CustomMerkleTree<T> = SparseMerkleTree::new(32);

    for (id, account) in accounts {
        let circuit_account = T::from_account(account.clone());

        account_state_tree.insert(*id as u32, circuit_account);
    }

    account_state_tree
}

type GenericSparseMerkeTree<T> = SparseMerkleTree<T, Fr, RescueHasher<Engine>>;

pub fn verify_identical_trees<T: Sync + GetBits + Default, S: Sync + GetBits + Default>(
    first_tree: &GenericSparseMerkeTree<T>,
    second_tree: &GenericSparseMerkeTree<S>,
    elements_to_check: u32,
    verify_equality: fn(u32, &T, &S) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    for index in 0..=elements_to_check {
        let first_tree_element = first_tree.get(index);
        let second_tree_element = second_tree.get(index);

        match (first_tree_element, second_tree_element) {
            (Some(first), Some(second)) => verify_equality(index, first, second)?,
            (Some(_), None) => {
                return Err(anyhow::format_err!(
                    "The second tree does not contain {}",
                    index
                ))
            }
            (None, Some(_)) => {
                return Err(anyhow::format_err!(
                    "The first tree does not contain {}",
                    index
                ))
            }
            (None, None) => continue, // None of the trees contain the element. That is ok
        }
    }

    Ok(())
}

pub fn verify_identical_balances(
    index: u32,
    first_balance: &Balance<Engine>,
    second_balance: &Balance<Engine>,
) -> anyhow::Result<()> {
    if first_balance.value == second_balance.value {
        Ok(())
    } else {
        Err(anyhow::format_err!(
            "Balance {} differs: first: {}, second:{}",
            index,
            first_balance.value,
            second_balance.value
        ))
    }
}

pub fn verify_accounts_equal<T: CircuitAccountWrapper, S: CircuitAccountWrapper>(
    index: u32,
    first_account: &T,
    second_account: &S,
) -> anyhow::Result<()> {
    let first_account = first_account.get_inner();
    let second_account = second_account.get_inner();

    if first_account.nonce != second_account.nonce {
        return Err(anyhow::format_err!(
            "The account {} have different nonces",
            index
        ));
    }
    if first_account.pub_key_hash != second_account.pub_key_hash {
        return Err(anyhow::format_err!(
            "The account {} have different pubKeyHash",
            index
        ));
    }
    if first_account.address != second_account.address {
        return Err(anyhow::format_err!(
            "The account {} have different address",
            index
        ));
    }

    verify_identical_trees(
        &first_account.subtree,
        &second_account.subtree,
        // It is better to hardcode the account tree size in one place
        // than to create a function which takes this as a param and returns another function
        NUMBER_OF_OLD_TOKENS,
        verify_identical_balances,
    )
    .map_err(|err| anyhow::format_err!("Account {}: {}", index, err))?;

    Ok(())
}
