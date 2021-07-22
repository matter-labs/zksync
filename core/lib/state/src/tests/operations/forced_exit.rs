use crate::tests::{AccountState::*, PlasmaTestBuilder};
use num::{BigUint, Zero};
use zksync_types::{account::AccountUpdate, tx::ForcedExit, AccountId, TokenId};

/// Check ForcedExit operation
#[test]
fn success() {
    let token_id = TokenId(0);
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (initiator_account_id, initiator_account, initiator_sk) = tb.add_account(Unlocked);
    let (target_account_id, target_account, _) = tb.add_account(Locked);

    tb.set_balance(initiator_account_id, token_id, fee.clone());
    tb.set_balance(target_account_id, token_id, amount.clone());

    let forced_exit = ForcedExit::new_signed(
        initiator_account_id,
        target_account.address,
        token_id,
        fee.clone(),
        initiator_account.nonce,
        Default::default(),
        &initiator_sk,
    )
    .unwrap();

    tb.test_tx_success(
        forced_exit.into(),
        &[
            (
                initiator_account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: initiator_account.nonce,
                    new_nonce: initiator_account.nonce + 1,
                    balance_update: (token_id, fee, BigUint::zero()),
                },
            ),
            (
                target_account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: target_account.nonce,
                    new_nonce: target_account.nonce,
                    balance_update: (token_id, amount, BigUint::zero()),
                },
            ),
        ],
    )
}

/// Check ForcedExit failure if target wallet is unlocked
#[test]
fn unlocked_target() {
    let token_id = TokenId(0);
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (initiator_account_id, initiator_account, initiator_sk) = tb.add_account(Unlocked);
    let (target_account_id, target_account, _) = tb.add_account(Unlocked);

    tb.set_balance(initiator_account_id, token_id, fee.clone());
    tb.set_balance(target_account_id, token_id, amount);

    let forced_exit = ForcedExit::new_signed(
        initiator_account_id,
        target_account.address,
        token_id,
        fee,
        initiator_account.nonce,
        Default::default(),
        &initiator_sk,
    )
    .unwrap();

    tb.test_tx_fail(
        forced_exit.into(),
        "Target account is not locked; forced exit is forbidden",
    );
}

/// Check ForcedExit failure if not enough funds
#[test]
fn insufficient_funds() {
    let token_id = TokenId(0);
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (initiator_account_id, initiator_account, initiator_sk) = tb.add_account(Unlocked);
    let (target_account_id, target_account, _) = tb.add_account(Locked);

    tb.set_balance(target_account_id, token_id, amount);

    let forced_exit = ForcedExit::new_signed(
        initiator_account_id,
        target_account.address,
        token_id,
        fee,
        initiator_account.nonce,
        Default::default(),
        &initiator_sk,
    )
    .unwrap();

    tb.test_tx_fail(
        forced_exit.into(),
        "Initiator account: Not enough balance to cover fees",
    );
}

/// Check ForcedExit failure if nonce is incorrect
#[test]
fn nonce_mismatch() {
    let token_id = TokenId(0);
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (initiator_account_id, initiator_account, initiator_sk) = tb.add_account(Unlocked);
    let (target_account_id, target_account, _) = tb.add_account(Locked);

    tb.set_balance(initiator_account_id, token_id, fee.clone());
    tb.set_balance(target_account_id, token_id, amount);

    let forced_exit = ForcedExit::new_signed(
        initiator_account_id,
        target_account.address,
        token_id,
        fee,
        initiator_account.nonce + 42,
        Default::default(),
        &initiator_sk,
    )
    .unwrap();

    tb.test_tx_fail(forced_exit.into(), "Nonce mismatch")
}

/// Check ForcedExit failure if account address
/// does not correspond to accound_id
#[test]
fn invalid_account_id() {
    let token_id = TokenId(0);
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (initiator_account_id, initiator_account, initiator_sk) = tb.add_account(Unlocked);
    let (target_account_id, target_account, _) = tb.add_account(Locked);

    tb.set_balance(initiator_account_id, token_id, fee.clone());
    tb.set_balance(target_account_id, token_id, amount);

    let forced_exit = ForcedExit::new_signed(
        AccountId(*initiator_account_id + 42),
        target_account.address,
        token_id,
        fee,
        initiator_account.nonce,
        Default::default(),
        &initiator_sk,
    )
    .unwrap();

    tb.test_tx_fail(forced_exit.into(), "Initiator account does not exist")
}
