use crate::tests::PlasmaTestBuilder;
use models::node::{AccountUpdate, Transfer};
use num::{BigUint, Zero};

#[test]
fn test_plasma_transfer_success() {
    let token_id = 0;
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (from_account_id, from_account, from_sk) = tb.add_account(true);
    tb.set_balance(from_account_id, token_id, &amount + &fee);

    let (to_account_id, to_account, _to_sk) = tb.add_account(false);

    let transfer = Transfer::new_signed(
        from_account_id,
        from_account.address,
        to_account.address,
        token_id,
        amount.clone(),
        fee.clone(),
        from_account.nonce,
        &from_sk,
    )
    .unwrap();

    tb.test_tx_success(
        transfer.into(),
        &[
            (
                from_account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: from_account.nonce,
                    new_nonce: from_account.nonce + 1,
                    balance_update: (token_id, &amount + &fee, BigUint::zero()),
                },
            ),
            (
                to_account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: to_account.nonce,
                    new_nonce: to_account.nonce,
                    balance_update: (token_id, BigUint::zero(), amount),
                },
            ),
        ],
    )
}

#[test]
fn test_plasma_transfer_insufficient() {
    let token_id = 0;
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (from_account_id, from_account, from_sk) = tb.add_account(true);
    tb.set_balance(from_account_id, token_id, amount.clone()); // balance is insufficient to pay fees

    let (_to_account_id, to_account, _to_sk) = tb.add_account(false);

    let transfer = Transfer::new_signed(
        from_account_id,
        from_account.address,
        to_account.address,
        token_id,
        amount,
        fee,
        from_account.nonce,
        &from_sk,
    )
    .unwrap();

    tb.test_tx_fail(transfer.into(), "Not enough balance");
}
