use crate::tests::{AccountState::*, PlasmaTestBuilder};
use num::{BigUint, Zero};
use web3::types::H256;
use zksync_crypto::params::MIN_NFT_TOKEN_ID;
use zksync_types::{account::AccountUpdate, tx::WithdrawNFT, AccountId, TokenId};

/// Check withdraw nft operation
#[test]
fn success() {
    let fee_token_id = TokenId(0);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (creator_account_id, _, _) = tb.add_account(Unlocked);
    let (account_id, account, sk) = tb.add_account(Unlocked);
    let content_hash = H256::random();
    let token_id = TokenId(MIN_NFT_TOKEN_ID);
    tb.set_balance(account_id, fee_token_id, fee.clone());
    tb.mint_nft(token_id, content_hash, account_id, creator_account_id);

    let withdraw = WithdrawNFT::new_signed(
        account_id,
        account.address,
        account.address,
        token_id,
        fee_token_id,
        fee.clone(),
        account.nonce,
        Default::default(),
        &sk,
    )
    .unwrap();

    tb.test_tx_success(
        withdraw.into(),
        &[
            (
                account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: account.nonce,
                    new_nonce: account.nonce + 1,
                    balance_update: (token_id, BigUint::from(1u32), BigUint::zero()),
                },
            ),
            (
                account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: account.nonce + 1,
                    new_nonce: account.nonce + 1,
                    balance_update: (fee_token_id, fee, BigUint::zero()),
                },
            ),
        ],
    )
}

/// Check Withdraw failure if not enough for paying fee
#[test]
fn insufficient_funds() {
    let fee_token_id = TokenId(0);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (creator_account_id, _, _) = tb.add_account(Unlocked);
    let (account_id, account, sk) = tb.add_account(Unlocked);
    let content_hash = H256::random();
    let token_id = TokenId(MIN_NFT_TOKEN_ID);
    tb.mint_nft(token_id, content_hash, account_id, creator_account_id);

    let withdraw = WithdrawNFT::new_signed(
        account_id,
        account.address,
        account.address,
        token_id,
        fee_token_id,
        fee,
        account.nonce,
        Default::default(),
        &sk,
    )
    .unwrap();

    tb.test_tx_fail(withdraw.into(), "Not enough balance");
}

#[test]
fn no_nft_on_balance() {
    let fee_token_id = TokenId(0);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (creator_account_id, creator_account, sk) = tb.add_account(Unlocked);
    let (account_id, _, _) = tb.add_account(Unlocked);
    let content_hash = H256::random();
    let token_id = TokenId(MIN_NFT_TOKEN_ID);
    tb.set_balance(creator_account_id, fee_token_id, fee.clone());
    tb.mint_nft(token_id, content_hash, account_id, creator_account_id);

    let withdraw = WithdrawNFT::new_signed(
        creator_account_id,
        creator_account.address,
        creator_account.address,
        token_id,
        fee_token_id,
        fee,
        creator_account.nonce,
        Default::default(),
        &sk,
    )
    .unwrap();

    tb.test_tx_fail(withdraw.into(), "Not enough nft balance");
}

#[test]
fn nft_does_not_exists() {
    let fee_token_id = TokenId(0);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (account_id, account, sk) = tb.add_account(Unlocked);
    let token_id = TokenId(MIN_NFT_TOKEN_ID);
    tb.set_balance(account_id, fee_token_id, fee.clone());

    let withdraw = WithdrawNFT::new_signed(
        account_id,
        account.address,
        account.address,
        token_id,
        fee_token_id,
        fee,
        account.nonce,
        Default::default(),
        &sk,
    )
    .unwrap();

    tb.test_tx_fail(withdraw.into(), "NFT was not found");
}

/// Check Withdraw NFT failure if nonce is incorrect
#[test]
fn nonce_mismatch() {
    let fee_token_id = TokenId(0);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (creator_account_id, _, _) = tb.add_account(Unlocked);
    let (account_id, account, sk) = tb.add_account(Unlocked);
    let content_hash = H256::random();
    let token_id = TokenId(MIN_NFT_TOKEN_ID);
    tb.set_balance(account_id, fee_token_id, fee.clone());
    tb.mint_nft(token_id, content_hash, account_id, creator_account_id);

    let withdraw = WithdrawNFT::new_signed(
        account_id,
        account.address,
        account.address,
        token_id,
        fee_token_id,
        fee,
        account.nonce + 10,
        Default::default(),
        &sk,
    )
    .unwrap();

    tb.test_tx_fail(withdraw.into(), "Nonce mismatch")
}

#[test]
fn invalid_account_id() {
    let fee_token_id = TokenId(0);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (creator_account_id, _, _) = tb.add_account(Unlocked);
    let (account_id, account, sk) = tb.add_account(Unlocked);
    let content_hash = H256::random();
    let token_id = TokenId(MIN_NFT_TOKEN_ID);
    tb.set_balance(account_id, fee_token_id, fee.clone());
    tb.mint_nft(token_id, content_hash, account_id, creator_account_id);

    let withdraw = WithdrawNFT::new_signed(
        AccountId(*account_id + 145),
        account.address,
        account.address,
        token_id,
        fee_token_id,
        fee,
        account.nonce + 10,
        Default::default(),
        &sk,
    )
    .unwrap();

    tb.test_tx_fail(withdraw.into(), "Withdraw account id is incorrect")
}
