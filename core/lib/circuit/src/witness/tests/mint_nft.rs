use num::BigUint;

use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_state::handler::TxHandler;
use zksync_state::state::{CollectedFee, ZkSyncState};
use zksync_types::{AccountId, MintNFT, MintNFTOp, TokenId, H256};

use crate::witness::tests::test_utils::{
    generic_test_scenario, incorrect_op_test_scenario, WitnessTestAccount,
};
use crate::witness::{MintNFTWitness, SigDataInput};
use zksync_crypto::params::{MIN_NFT_TOKEN_ID, NFT_STORAGE_ACCOUNT_ID, NFT_TOKEN_ID};

/// Basic check for execution of `MintNFT` operation in circuit.
/// Here we create two accounts and perform a mintNFT operation.
#[test]
#[ignore]
fn test_mint_nft_success() {
    // Test vector of (initial_balance, fee_amount).
    let test_vector = vec![(10u64, 3u64)];

    let content_hash = H256::random();
    for (initial_balance, fee_amount) in test_vector {
        // Input data.
        let accounts = vec![
            WitnessTestAccount::new(AccountId(1), initial_balance),
            WitnessTestAccount::new_empty(AccountId(2)),
            WitnessTestAccount::new_with_token(
                NFT_STORAGE_ACCOUNT_ID,
                NFT_TOKEN_ID,
                MIN_NFT_TOKEN_ID as u64,
            ),
        ];
        let (account_from, account_to) = (&accounts[0], &accounts[1]);
        let mint_nft_op = MintNFTOp {
            tx: account_from
                .zksync_account
                .sign_mint_nft(
                    TokenId(0),
                    "",
                    content_hash,
                    BigUint::from(fee_amount),
                    &account_to.account.address,
                    None,
                    true,
                )
                .0,
            creator_account_id: account_from.id,
            recipient_account_id: account_to.id,
        };

        // Additional data required for performing the operation.
        let input =
            SigDataInput::from_mint_nft_op(&mint_nft_op).expect("SigDataInput creation failed");

        generic_test_scenario::<MintNFTWitness<Bn256>, _>(
            &accounts,
            mint_nft_op,
            input,
            |plasma_state, op| {
                let fee = <ZkSyncState as TxHandler<MintNFT>>::apply_op(plasma_state, &op)
                    .expect("Operation failed")
                    .0
                    .unwrap();
                vec![fee]
            },
        );
    }
}

/// Checks that executing a mintNFT operation with incorrect
/// data (insufficient funds) results in an error.
#[test]
#[ignore]
fn test_mint_nft_incorrect_fee() {
    // Balance check should fail.
    // "balance-fee bits" is message for subtraction check in circuit.
    // For details see `circuit.rs`.
    const ERR_MSG: &str = "balance-fee bits";

    // Test vector of (initial_balance, fee_amount).
    let test_vector = vec![(10u64, 11u64), (15u64, 119u64)];

    let content_hash = H256::random();
    for (initial_balance, fee_amount) in test_vector {
        // Input data.
        let accounts = vec![
            WitnessTestAccount::new(AccountId(1), initial_balance),
            WitnessTestAccount::new_empty(AccountId(2)),
            WitnessTestAccount::new_with_token(
                NFT_STORAGE_ACCOUNT_ID,
                NFT_TOKEN_ID,
                MIN_NFT_TOKEN_ID as u64,
            ),
        ];
        let (account_from, account_to) = (&accounts[0], &accounts[1]);
        let mint_nft_op = MintNFTOp {
            tx: account_from
                .zksync_account
                .sign_mint_nft(
                    TokenId(0),
                    "",
                    content_hash,
                    BigUint::from(fee_amount),
                    &account_to.account.address,
                    None,
                    true,
                )
                .0,
            creator_account_id: account_from.id,
            recipient_account_id: account_to.id,
        };

        // Additional data required for performing the operation.
        let input =
            SigDataInput::from_mint_nft_op(&mint_nft_op).expect("SigDataInput creation failed");

        incorrect_op_test_scenario::<MintNFTWitness<Bn256>, _>(
            &accounts,
            mint_nft_op,
            input,
            ERR_MSG,
            || {
                vec![CollectedFee {
                    token: TokenId(0),
                    amount: fee_amount.into(),
                }]
            },
        );
    }
}
