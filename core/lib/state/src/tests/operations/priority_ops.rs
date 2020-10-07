use crate::tests::{AccountState::*, PlasmaTestBuilder};
use num::{BigUint, Zero};
use web3::types::H160;
use zksync_types::priority_ops::{Deposit, FullExit};
use zksync_types::{account::AccountUpdate, ZkSyncPriorityOp};

/// Check Deposit to existing account
#[test]
fn deposit_to_existing() {
    let token = 0;
    let amount = BigUint::from(100u32);
    let mut tb = PlasmaTestBuilder::new();
    let (account_id, account, _) = tb.add_account(Locked);

    let deposit = Deposit {
        from: account.address,
        to: account.address,
        amount,
        token,
    };

    tb.test_priority_op_success(
        ZkSyncPriorityOp::Deposit(deposit),
        &[(
            account_id,
            AccountUpdate::UpdateBalance {
                old_nonce: account.nonce,
                new_nonce: account.nonce,
                balance_update: (token, BigUint::zero(), BigUint::from(100u32)),
            },
        )],
    )
}

/// Check Deposit to new account
#[test]
fn deposit_to_new() {
    let token = 0;
    let amount = BigUint::from(100u32);
    let mut tb = PlasmaTestBuilder::new();
    let address = H160::random();
    let account_id = tb.state.get_free_account_id();

    let deposit = Deposit {
        from: address,
        to: address,
        amount,
        token,
    };

    tb.test_priority_op_success(
        ZkSyncPriorityOp::Deposit(deposit),
        &[
            (account_id, AccountUpdate::Create { address, nonce: 0 }),
            (
                account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: 0,
                    new_nonce: 0,
                    balance_update: (token, BigUint::zero(), BigUint::from(100u32)),
                },
            ),
        ],
    )
}

/// Check failure of FullExit operation for non-existent account
#[test]
fn full_exit_non_existent() {
    let token = 0;
    let eth_address = H160::random();
    let mut tb = PlasmaTestBuilder::new();

    let full_exit = FullExit {
        token,
        eth_address,
        account_id: 145,
    };

    tb.test_priority_op_success(ZkSyncPriorityOp::FullExit(full_exit), &[])
}

/// Check successfull FullExit
#[test]
fn full_exit_success() {
    let token = 0;
    let amount = BigUint::from(145u32);
    let mut tb = PlasmaTestBuilder::new();
    let (account_id, account, _) = tb.add_account(Locked);
    tb.set_balance(account_id, token, amount.clone());

    let full_exit = FullExit {
        token,
        eth_address: account.address,
        account_id,
    };

    tb.test_priority_op_success(
        ZkSyncPriorityOp::FullExit(full_exit),
        &[(
            account_id,
            AccountUpdate::UpdateBalance {
                old_nonce: account.nonce,
                new_nonce: account.nonce,
                balance_update: (token, amount, BigUint::zero()),
            },
        )],
    )
}
