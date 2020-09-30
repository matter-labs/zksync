use super::{AccountState::*, PlasmaTestBuilder};
use crate::state::CollectedFee;
use num::{BigUint, Zero};
use zksync_types::account::AccountUpdate;

/// Checks if fees are collected correctly.
/// Fees are not only in ETH and may be zero.
#[test]
fn success() {
    let mut tb = PlasmaTestBuilder::new();
    let (account_id, account, _) = tb.add_account(Locked);
    tb.set_balance(account_id, 0, 145u32);

    let nonce = account.nonce;
    let mut state_clone = tb.state.clone();

    let actual_updates = tb.state.collect_fee(
        &[
            CollectedFee {
                token: 0,
                amount: BigUint::from(145u32),
            },
            CollectedFee {
                token: 1,
                amount: BigUint::from(0u32),
            },
            CollectedFee {
                token: 2,
                amount: BigUint::from(123456u32),
            },
        ],
        account_id,
    );

    let expected_updates = [
        (
            account_id,
            AccountUpdate::UpdateBalance {
                old_nonce: nonce,
                new_nonce: nonce,
                balance_update: (0, BigUint::from(145u32), BigUint::from(290u32)),
            },
        ),
        (
            account_id,
            AccountUpdate::UpdateBalance {
                old_nonce: nonce,
                new_nonce: nonce,
                balance_update: (2, BigUint::zero(), BigUint::from(123456u32)),
            },
        ),
    ];

    tb.compare_updates(&expected_updates, &actual_updates, &mut state_clone)
}

/// Checks that collect_fee panics if the collecting account does not exist.
#[test]
#[should_panic(expected = "Fee account should be present in the account tree: 145")]
fn invalid_account() {
    let mut tb = PlasmaTestBuilder::new();
    tb.state.collect_fee(&[], 145);
}
