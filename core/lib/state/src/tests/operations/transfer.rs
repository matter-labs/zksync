use crate::tests::{AccountState::*, PlasmaTestBuilder};
use num::{BigUint, Zero};
use web3::types::H160;
use zksync_types::{AccountId, AccountUpdate, Nonce, SignedZkSyncTx, TokenId, Transfer, ZkSyncTx};

/// Check Transfer operation to existing account
#[test]
fn to_existing() {
    let token_id = TokenId(0);
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (from_account_id, from_account, from_sk) = tb.add_account(Unlocked);
    tb.set_balance(from_account_id, token_id, &amount + &fee);

    let (to_account_id, to_account, _to_sk) = tb.add_account(Locked);

    let transfer = Transfer::new_signed(
        from_account_id,
        from_account.address,
        to_account.address,
        token_id,
        amount.clone(),
        fee.clone(),
        from_account.nonce,
        Default::default(),
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

/// Check Transfer failure if not enough funds
#[test]
fn insufficient_funds() {
    let token_id = TokenId(0);
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (from_account_id, from_account, from_sk) = tb.add_account(Unlocked);
    tb.set_balance(from_account_id, token_id, amount.clone()); // balance is insufficient to pay fees

    let (_to_account_id, to_account, _to_sk) = tb.add_account(Locked);

    let transfer = Transfer::new_signed(
        from_account_id,
        from_account.address,
        to_account.address,
        token_id,
        amount,
        fee,
        from_account.nonce,
        Default::default(),
        &from_sk,
    )
    .unwrap();

    tb.test_tx_fail(transfer.into(), "Not enough balance");
}

/// Check Transfer operation to new account
#[test]
fn to_new() {
    let token_id = TokenId(0);
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (account_id, account, sk) = tb.add_account(Unlocked);
    tb.set_balance(account_id, token_id, &amount + &fee);

    let new_address = H160::random();
    let new_id = tb.state.get_free_account_id();

    let transfer = Transfer::new_signed(
        account_id,
        account.address,
        new_address,
        token_id,
        amount.clone(),
        fee.clone(),
        account.nonce,
        Default::default(),
        &sk,
    )
    .unwrap();

    tb.test_tx_success(
        transfer.into(),
        &[
            (
                new_id,
                AccountUpdate::Create {
                    address: new_address,
                    nonce: Nonce(0),
                },
            ),
            (
                account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: account.nonce,
                    new_nonce: account.nonce + 1,
                    balance_update: (token_id, &amount + &fee, BigUint::zero()),
                },
            ),
            (
                new_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: Nonce(0),
                    new_nonce: Nonce(0),
                    balance_update: (token_id, BigUint::zero(), amount),
                },
            ),
        ],
    )
}

/// Check Transfer operation to new account
#[test]
fn to_new_failure() {
    let token_id = TokenId(0);
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (account_id, account, sk) = tb.add_account(Unlocked);
    tb.set_balance(account_id, token_id, &amount + &fee);
    let too_big_amount = amount * 2u64;

    let new_address = H160::random();

    let transfer = Transfer::new_signed(
        account_id,
        account.address,
        new_address,
        token_id,
        too_big_amount,
        fee,
        account.nonce,
        Default::default(),
        &sk,
    )
    .unwrap();

    tb.test_tx_fail(transfer.into(), "Not enough balance")
}

/// Check Transfer operation from account to itself
#[test]
fn to_self() {
    let token_id = TokenId(0);
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (account_id, account, sk) = tb.add_account(Unlocked);
    tb.set_balance(account_id, token_id, &amount + &fee);

    let transfer = Transfer::new_signed(
        account_id,
        account.address,
        account.address,
        token_id,
        amount.clone(),
        fee.clone(),
        account.nonce,
        Default::default(),
        &sk,
    )
    .unwrap();

    tb.test_tx_success(
        transfer.into(),
        &[(
            account_id,
            AccountUpdate::UpdateBalance {
                old_nonce: account.nonce,
                new_nonce: account.nonce + 1,
                balance_update: (token_id, &amount + &fee, amount),
            },
        )],
    )
}

/// Check Transfer failure if nonce is incorrect
#[test]
fn nonce_mismatch() {
    let token_id = TokenId(0);
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (account_id, account, sk) = tb.add_account(Unlocked);
    tb.set_balance(account_id, token_id, &amount + &fee);

    let transfer = Transfer::new_signed(
        account_id,
        account.address,
        account.address,
        token_id,
        amount,
        fee,
        account.nonce + 1,
        Default::default(),
        &sk,
    )
    .unwrap();

    tb.test_tx_fail(transfer.into(), "Nonce mismatch")
}

/// Check Transfer failure if account address
/// does not correspond to accound_id
#[test]
fn invalid_account_id() {
    let token_id = TokenId(0);
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (account_id, account, sk) = tb.add_account(Unlocked);
    let (_, to_account, _) = tb.add_account(Locked);
    tb.set_balance(account_id, token_id, &amount + &fee);

    let transfer = Transfer::new_signed(
        AccountId(*account_id + 145),
        account.address,
        to_account.address,
        token_id,
        amount,
        fee,
        account.nonce,
        Default::default(),
        &sk,
    )
    .unwrap();

    tb.test_tx_fail(transfer.into(), "Transfer account id is incorrect")
}

#[test]
fn execute_txs_batch_success_transfers() {
    let token_id = TokenId(0);
    let amount = BigUint::from(100u32);
    let fee = BigUint::from(10u32);
    let small_amount = BigUint::from(10u32);

    let balance_from = &fee + &amount;
    let balance_from_after_first = &balance_from - &small_amount - &fee;
    let balance_from_final = &balance_from_after_first - &small_amount - &fee;

    let balance_to = 0u64.into();
    let balance_to_after_first = &balance_to + &small_amount;
    let balance_to_final = &balance_to_after_first + &small_amount;

    let mut tb = PlasmaTestBuilder::new();

    let (account_id, account, sk) = tb.add_account(Unlocked);
    tb.set_balance(account_id, token_id, &amount + &fee);

    let new_address = H160::random();

    let transfer_1 = Transfer::new_signed(
        account_id,
        account.address,
        new_address,
        token_id,
        small_amount.clone(),
        fee.clone(),
        account.nonce,
        Default::default(),
        &sk,
    )
    .unwrap();

    let transfer_2 = Transfer::new_signed(
        account_id,
        account.address,
        new_address,
        token_id,
        small_amount,
        fee.clone(),
        account.nonce + 1,
        Default::default(),
        &sk,
    )
    .unwrap();

    let transfer_bad = Transfer::new_signed(
        account_id,
        account.address,
        new_address,
        token_id,
        amount * 2u64,
        fee,
        account.nonce + 1,
        Default::default(),
        &sk,
    )
    .unwrap();

    let signed_zk_sync_tx1 = SignedZkSyncTx {
        tx: ZkSyncTx::Transfer(Box::new(transfer_1)),
        eth_sign_data: None,
    };
    let signed_zk_sync_tx2 = SignedZkSyncTx {
        tx: ZkSyncTx::Transfer(Box::new(transfer_2)),
        eth_sign_data: None,
    };
    let signed_zk_sync_tx_bad = SignedZkSyncTx {
        tx: ZkSyncTx::Transfer(Box::new(transfer_bad)),
        eth_sign_data: None,
    };

    tb.test_txs_batch_fail(
        &[signed_zk_sync_tx1.clone(), signed_zk_sync_tx_bad],
        "Batch execution failed, since tx #2 of batch failed with a reason: Not enough balance",
    );

    let new_id = tb.state.get_free_account_id();

    tb.test_txs_batch_success(
        &[signed_zk_sync_tx1, signed_zk_sync_tx2],
        &[
            (
                new_id,
                AccountUpdate::Create {
                    address: new_address,
                    nonce: Nonce(0),
                },
            ),
            (
                account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: account.nonce,
                    new_nonce: account.nonce + 1,
                    balance_update: (token_id, balance_from, balance_from_after_first.clone()),
                },
            ),
            (
                new_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: Nonce(0),
                    new_nonce: Nonce(0),
                    balance_update: (token_id, balance_to, balance_to_after_first.clone()),
                },
            ),
            (
                account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: account.nonce + 1,
                    new_nonce: account.nonce + 2,
                    balance_update: (token_id, balance_from_after_first, balance_from_final),
                },
            ),
            (
                new_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: Nonce(0),
                    new_nonce: Nonce(0),
                    balance_update: (token_id, balance_to_after_first, balance_to_final),
                },
            ),
        ],
    );
}
