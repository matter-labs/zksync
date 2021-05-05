use num::BigUint;

use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
use zksync_state::handler::TxHandler;
use zksync_state::state::{CollectedFee, ZkSyncState};
use zksync_types::{AccountId, MintNFT, MintNFTOp, TokenId, H256};

use crate::witness::tests::test_utils::{
    corrupted_input_test_scenario, generic_test_scenario, incorrect_fr, incorrect_op_test_scenario,
    WitnessTestAccount,
};
use crate::witness::{utils::WitnessBuilder, MintNFTWitness, SigDataInput};
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

        incorrect_op_test_scenario::<MintNFTWitness<Bn256>, _, _>(
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
            |_| {},
        );
    }
}

/// Checks that executing a mintNFT operation with incorrect
/// serial_id results in an error.
#[test]
#[ignore]
fn test_mint_nft_incorrect_serial_id() {
    // valid_serial_id variable in circuit should be false when incorrect serial_id
    const ERR_MSG: &str = "chunk number 1/execute_op/op_valid is true/enforce equal to one";

    let content_hash = H256::random();
    // Input data.
    let accounts = vec![
        WitnessTestAccount::new(AccountId(1), 10u64),
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
                BigUint::from(3u64),
                &account_to.account.address,
                None,
                true,
            )
            .0,
        creator_account_id: account_from.id,
        recipient_account_id: account_to.id,
    };

    // Additional data required for performing the operation.
    let input = SigDataInput::from_mint_nft_op(&mint_nft_op).expect("SigDataInput creation failed");

    let incorrect_serial_id = incorrect_fr();

    incorrect_op_test_scenario::<MintNFTWitness<Bn256>, _, _>(
        &accounts,
        mint_nft_op,
        input,
        ERR_MSG,
        || {
            vec![CollectedFee {
                token: TokenId(0),
                amount: 3u64.into(),
            }]
        },
        |builder: &mut WitnessBuilder| {
            for operation in builder.operations.iter_mut() {
                operation.args.special_serial_id = Some(incorrect_serial_id);
            }
        },
    );
}

/// Checks that executing a mintNFT operation with incorrect
/// new_token_id results in an error.
#[test]
#[ignore]
fn test_mint_nft_incorrect_new_token_id() {
    // is_new_token_id_valid variable in circuit should be false when incorrect new_token_id
    const ERR_MSG: &str = "chunk number 2/execute_op/op_valid is true/enforce equal to one";

    let content_hash = H256::random();
    // Input data.
    let accounts = vec![
        WitnessTestAccount::new(AccountId(1), 10u64),
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
                BigUint::from(3u64),
                &account_to.account.address,
                None,
                true,
            )
            .0,
        creator_account_id: account_from.id,
        recipient_account_id: account_to.id,
    };

    // Additional data required for performing the operation.
    let input = SigDataInput::from_mint_nft_op(&mint_nft_op).expect("SigDataInput creation failed");

    let incorrect_new_token_id = incorrect_fr();

    incorrect_op_test_scenario::<MintNFTWitness<Bn256>, _, _>(
        &accounts,
        mint_nft_op,
        input,
        ERR_MSG,
        || {
            vec![CollectedFee {
                token: TokenId(0),
                amount: 3u64.into(),
            }]
        },
        |builder: &mut WitnessBuilder| {
            for operation in builder.operations.iter_mut() {
                operation.args.special_tokens[1] = Some(incorrect_new_token_id);
            }
        },
    );
}

/// Checks that executing a mintNFT operation with
/// new_token_id equals to NFT_TOKEN_ID results in an error.
#[test]
#[ignore]
fn test_mint_nft_all_nft_slots_filled() {
    // is_special_nft_token.not() flag presents in fourth chunk in circuit
    const ERR_MSG: &str = "chunk number 3/execute_op/op_valid is true/enforce equal to one";

    let content_hash = H256::random();
    // Input data.
    let accounts = vec![
        WitnessTestAccount::new(AccountId(1), 10u64),
        WitnessTestAccount::new_empty(AccountId(2)),
        WitnessTestAccount::new_with_token(
            NFT_STORAGE_ACCOUNT_ID,
            NFT_TOKEN_ID,
            *NFT_TOKEN_ID as u64, // special value - let's imagine we have already filled all NFT slots
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
                BigUint::from(3u64),
                &account_to.account.address,
                None,
                true,
            )
            .0,
        creator_account_id: account_from.id,
        recipient_account_id: account_to.id,
    };

    // Additional data required for performing the operation.
    let input = SigDataInput::from_mint_nft_op(&mint_nft_op).expect("SigDataInput creation failed");

    incorrect_op_test_scenario::<MintNFTWitness<Bn256>, _, _>(
        &accounts,
        mint_nft_op,
        input,
        ERR_MSG,
        || {
            vec![CollectedFee {
                token: TokenId(0),
                amount: 3u64.into(),
            }]
        },
        |_| {},
    );
}

/// Checks that corrupted signature data leads to unsatisfied constraints in circuit.
#[test]
#[ignore]
fn test_mint_nft_corrupted_ops_input() {
    // Incorrect signature data will lead to `op_valid` constraint failure.
    // See `circuit.rs` for details.
    const ERR_MSG: &str = "chunk number 0/execute_op/op_valid is true/enforce equal to one";

    let content_hash = H256::random();
    // Input data.
    let accounts = vec![
        WitnessTestAccount::new(AccountId(1), 10u64),
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
                BigUint::from(3u64),
                &account_to.account.address,
                None,
                true,
            )
            .0,
        creator_account_id: account_from.id,
        recipient_account_id: account_to.id,
    };

    // Additional data required for performing the operation.
    let input = SigDataInput::from_mint_nft_op(&mint_nft_op).expect("SigDataInput creation failed");

    // Test vector with values corrupted one by one.
    let test_vector = input.corrupted_variations();

    for input in test_vector {
        corrupted_input_test_scenario::<MintNFTWitness<Bn256>, _, _>(
            &accounts,
            mint_nft_op.clone(),
            input,
            ERR_MSG,
            |plasma_state, op| {
                let fee = <ZkSyncState as TxHandler<MintNFT>>::apply_op(plasma_state, &op)
                    .expect("Operation failed")
                    .0
                    .unwrap();
                vec![fee]
            },
            |_| {},
        );
    }
}
