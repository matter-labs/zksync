use zksync_crypto::{
    circuit::account::{Balance, CircuitAccount, CircuitAccountTree, CircuitBalanceTree},
    merkle_tree::RescueHasher,
    merkle_tree::SparseMerkleTree,
    primitives::GetBits,
    Engine, Fr,
};

use crate::account::FromAccount;
use once_cell::sync::Lazy;
use zksync_types::Account;

pub static BALANCE_TREE_32: Lazy<CircuitBalanceTree> = Lazy::new(|| SparseMerkleTree::new(32));
pub static BALANCE_TREE_11: Lazy<CircuitBalanceTree> = Lazy::new(|| SparseMerkleTree::new(11));

pub fn get_state(
    accounts: &[(i64, Account)],
    balance_tree: &CircuitBalanceTree,
) -> CircuitAccountTree {
    let mut account_state_tree: CircuitAccountTree = SparseMerkleTree::new(32);

    for (id, account) in accounts {
        let circuit_account = CircuitAccount::from_account(account.clone(), &balance_tree);

        account_state_tree.insert(*id as u32, circuit_account);
    }

    account_state_tree
}

type GenericSparseMerkeTree<T> = SparseMerkleTree<T, Fr, RescueHasher<Engine>>;

pub fn verify_identical_trees<T: GetBits + Default + Sync>(
    first_tree: &GenericSparseMerkeTree<T>,
    second_tree: &GenericSparseMerkeTree<T>,
    elements_to_check: u32,
    verify_equality: fn(u32, &T, &T) -> anyhow::Result<()>,
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

pub fn verify_accounts_equal(
    index: u32,
    first_account: &CircuitAccount<Engine>,
    second_account: &CircuitAccount<Engine>,
) -> anyhow::Result<()> {
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

    // It is better to hardcode the account tree depth in one place
    // the to create a function which takes this as a param and returns another function
    let elements_to_check = 2u32.pow(11);

    verify_identical_trees(
        &first_account.subtree,
        &second_account.subtree,
        elements_to_check,
        verify_identical_balances,
    )
    .map_err(|err| anyhow::format_err!("Account {}: {}", index, err))?;

    Ok(())
}

// pub fn verify_identical_accounts(tree_depth_11: &CircuitAccountTree, tree_depth_32: &CircuitAccountTree) -> anyhow::Result<()> {
//     for index in 0..=u32::MAX {
//         let first_tree_element = tree_depth_11.get(index).ok_or_else(|| anyhow::format_err!("The first tree does not contain account {}", index))?;
//         let second_tree_element = tree_depth_32.get(index).ok_or_else(|| anyhow::format_err!("The second tree does not contain account {}", index))?;

//         let are_equal = verify_accounts_equal(first_tree_element, second_tree_element);

//         if !are_equal {
//             return Err(anyhow::format_err!("Account {} is different in the second tree", index));
//         }
//     }

//     Ok(())
// }
