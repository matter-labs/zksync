use num::{BigUint, Zero};
use web3::types::H256;

use zksync_crypto::params::{
    MIN_NFT_TOKEN_ID, NFT_STORAGE_ACCOUNT_ADDRESS, NFT_STORAGE_ACCOUNT_ID, NFT_TOKEN_ID,
};
use zksync_types::{tokens::NFT, AccountUpdate, Address, MintNFT, Nonce, TokenId};

use crate::tests::{AccountState::*, PlasmaTestBuilder};

/// Check MintNFT operation
#[test]
fn mint_success() {
    let fee_token_id = TokenId(0);
    let fee = BigUint::from(10u32);

    let mut tb = PlasmaTestBuilder::new();

    let (creator_account_id, mut creator_account, creator_sk) = tb.add_account(Unlocked);
    tb.set_balance(creator_account_id, fee_token_id, 20u32);

    let (to_account_id, to_account, _to_sk) = tb.add_account(Locked);
    let content_hash = H256::default();
    let mint_nft = MintNFT::new_signed(
        creator_account_id,
        creator_account.address,
        content_hash,
        to_account.address,
        fee.clone(),
        fee_token_id,
        creator_account.nonce,
        Default::default(),
        &creator_sk,
    )
    .unwrap();

    let token_hash: Vec<u8> = vec![
        0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let token_address = Address::from_slice(&token_hash[..20]);
    let balance = BigUint::from(MIN_NFT_TOKEN_ID);
    let nft = NFT::new(
        TokenId(MIN_NFT_TOKEN_ID + 1),
        1,
        creator_account_id,
        token_address,
        None,
        content_hash,
    );

    let token_data = BigUint::from_bytes_be(&token_hash[..16]);
    tb.test_tx_success(
        mint_nft.into(),
        &[
            (
                NFT_STORAGE_ACCOUNT_ID,
                AccountUpdate::Create {
                    address: *NFT_STORAGE_ACCOUNT_ADDRESS,
                    nonce: Nonce(0),
                },
            ),
            (
                NFT_STORAGE_ACCOUNT_ID,
                AccountUpdate::UpdateBalance {
                    old_nonce: Nonce(0),
                    new_nonce: Nonce(0),
                    balance_update: (NFT_TOKEN_ID, BigUint::zero(), balance),
                },
            ),
            (
                NFT_STORAGE_ACCOUNT_ID,
                AccountUpdate::UpdateBalance {
                    old_nonce: Nonce(0),
                    new_nonce: Nonce(0),
                    balance_update: (
                        NFT_TOKEN_ID,
                        BigUint::from(MIN_NFT_TOKEN_ID),
                        BigUint::from(MIN_NFT_TOKEN_ID + 1),
                    ),
                },
            ),
            (
                creator_account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: creator_account.nonce,
                    new_nonce: creator_account.nonce,
                    balance_update: (fee_token_id, BigUint::from(20u32), BigUint::from(10u32)),
                },
            ),
            (
                creator_account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: creator_account.nonce,
                    new_nonce: creator_account.nonce + 1,
                    balance_update: (NFT_TOKEN_ID, BigUint::zero(), BigUint::from(1u32)),
                },
            ),
            (
                creator_account_id,
                AccountUpdate::MintNFT { token: nft.clone() },
            ),
            (
                to_account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: to_account.nonce,
                    new_nonce: to_account.nonce,
                    balance_update: (nft.id, BigUint::zero(), BigUint::from(1u32)),
                },
            ),
            (
                NFT_STORAGE_ACCOUNT_ID,
                AccountUpdate::UpdateBalance {
                    old_nonce: to_account.nonce,
                    new_nonce: to_account.nonce,
                    balance_update: (nft.id, BigUint::zero(), token_data),
                },
            ),
        ],
    );

    // Create another nft
    creator_account.nonce.0 += 1;
    let (to_account_id, to_account, _to_sk) = tb.add_account(Locked);
    let content_hash = H256::default();
    let mint_nft = MintNFT::new_signed(
        creator_account_id,
        creator_account.address,
        content_hash,
        to_account.address,
        fee.clone(),
        fee_token_id,
        creator_account.nonce,
        Default::default(),
        &creator_sk,
    )
    .unwrap();

    let token_hash: Vec<u8> = vec![
        0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let token_address = Address::from_slice(&token_hash[..20]);
    let nft = NFT::new(
        TokenId(MIN_NFT_TOKEN_ID + 2),
        2,
        creator_account_id,
        token_address,
        None,
        content_hash,
    );

    let token_data = BigUint::from_bytes_be(&token_hash[..16]);
    tb.test_tx_success(
        mint_nft.into(),
        &[
            (
                NFT_STORAGE_ACCOUNT_ID,
                AccountUpdate::UpdateBalance {
                    old_nonce: Nonce(0),
                    new_nonce: Nonce(0),
                    balance_update: (
                        NFT_TOKEN_ID,
                        BigUint::from(MIN_NFT_TOKEN_ID + 1),
                        BigUint::from(MIN_NFT_TOKEN_ID + 2),
                    ),
                },
            ),
            (
                creator_account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: creator_account.nonce,
                    new_nonce: creator_account.nonce,
                    balance_update: (fee_token_id, fee, BigUint::zero()),
                },
            ),
            (
                creator_account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: creator_account.nonce,
                    new_nonce: creator_account.nonce + 1,
                    balance_update: (NFT_TOKEN_ID, BigUint::from(1u32), BigUint::from(2u32)),
                },
            ),
            (
                creator_account_id,
                AccountUpdate::MintNFT { token: nft.clone() },
            ),
            (
                to_account_id,
                AccountUpdate::UpdateBalance {
                    old_nonce: to_account.nonce,
                    new_nonce: to_account.nonce,
                    balance_update: (nft.id, BigUint::zero(), BigUint::from(1u32)),
                },
            ),
            (
                NFT_STORAGE_ACCOUNT_ID,
                AccountUpdate::UpdateBalance {
                    old_nonce: to_account.nonce,
                    new_nonce: to_account.nonce,
                    balance_update: (nft.id, BigUint::zero(), token_data),
                },
            ),
        ],
    )
}
