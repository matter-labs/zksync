use crate::tests::PlasmaTestBuilder;
use models::{account::AccountUpdate, tx::Withdraw};
use num::{BigUint, Zero};

#[test]
fn success() {
    let token_id = 0;
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (account_id, account, sk) = tb.add_account(true);
    tb.set_balance(account_id, token_id, &amount + &fee);

    let withdraw = Withdraw::new_signed(
        account_id,
        account.address,
        account.address,
        token_id,
        amount.clone(),
        fee.clone(),
        account.nonce,
        &sk,
    )
    .unwrap();

    tb.test_tx_success(
        withdraw.into(),
        &[(
            account_id,
            AccountUpdate::UpdateBalance {
                old_nonce: account.nonce,
                new_nonce: account.nonce + 1,
                balance_update: (token_id, &amount + &fee, BigUint::zero()),
            },
        )],
    )
}

#[test]
fn insufficient_funds() {
    let token_id = 0;
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (account_id, account, sk) = tb.add_account(true);
    tb.set_balance(account_id, token_id, amount.clone());

    let withdraw = Withdraw::new_signed(
        account_id,
        account.address,
        account.address,
        token_id,
        amount,
        fee,
        account.nonce,
        &sk,
    )
    .unwrap();

    tb.test_tx_fail(withdraw.into(), "Not enough balance");
}

#[test]
fn nonce_mismatch() {
    let token_id = 0;
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (account_id, account, sk) = tb.add_account(true);
    tb.set_balance(account_id, token_id, &amount + &fee);

    let withdraw = Withdraw::new_signed(
        account_id,
        account.address,
        account.address,
        token_id,
        amount,
        fee,
        account.nonce + 1,
        &sk,
    )
    .unwrap();

    tb.test_tx_fail(withdraw.into(), "Nonce mismatch")
}

#[test]
fn invalid_account_id() {
    let token_id = 0;
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (account_id, account, sk) = tb.add_account(true);
    let (_, to_account, _) = tb.add_account(true);
    tb.set_balance(account_id, token_id, &amount + &fee);

    let withdraw = Withdraw::new_signed(
        account_id + 145,
        account.address,
        to_account.address,
        token_id,
        amount,
        fee,
        account.nonce,
        &sk,
    )
    .unwrap();

    tb.test_tx_fail(withdraw.into(), "Withdraw account id is incorrect")
}
