// External deps
use num::BigUint;
use zksync_crypto::franklin_crypto::bellman::pairing::{
    bn256::{Bn256, Fr},
    ff::Field,
};
// Workspace deps
use zksync_state::{handler::TxHandler, state::ZkSyncState};
use zksync_types::{
    operations::FullExitOp, AccountId, BlockNumber, FullExit, MintNFT, MintNFTOp, TokenId, H256,
};
// Local deps
use crate::{
    circuit::ZkSyncCircuit,
    witness::{
        full_exit::FullExitWitness,
        tests::test_utils::{
            check_circuit, check_circuit_non_panicking, generic_test_scenario, incorrect_fr,
            incorrect_op_test_scenario, WitnessTestAccount, ZkSyncStateGenerator, FEE_ACCOUNT_ID,
        },
        utils::WitnessBuilder,
        MintNFTWitness, SigDataInput, Witness,
    },
};
use zksync_crypto::params::{
    CONTENT_HASH_WIDTH, MIN_NFT_TOKEN_ID, NFT_STORAGE_ACCOUNT_ID, NFT_TOKEN_ID,
};

/// Checks that `FullExit` can be applied to an existing account.
/// Here we generate a ZkSyncState with one account (which has some funds), and
/// apply a `FullExit` to this account.
#[test]
#[ignore]
fn test_full_exit_success() {
    // Input data.
    let accounts = vec![WitnessTestAccount::new(AccountId(1), 10)];
    let account = &accounts[0];
    let full_exit_op = FullExitOp {
        priority_op: FullExit {
            account_id: account.id,
            eth_address: account.account.address,
            token: TokenId(0),
        },
        withdraw_amount: Some(BigUint::from(10u32).into()),
        creator_account_id: None,
        creator_address: None,
        serial_id: None,
        content_hash: None,
    };
    let success = true;

    generic_test_scenario::<FullExitWitness<Bn256>, _>(
        &accounts,
        (full_exit_op, success),
        (),
        |plasma_state, op| {
            <ZkSyncState as TxHandler<FullExit>>::apply_op(plasma_state, &op.0)
                .expect("FullExit failed");
            vec![]
        },
    );
}

fn apply_nft_mint_and_full_exit_nft_operations() -> ZkSyncCircuit<'static, Bn256> {
    let accounts = vec![
        WitnessTestAccount::new(AccountId(1), 10u64), // nft creator account
        WitnessTestAccount::new(AccountId(2), 10u64), // account to withdraw nft
        WitnessTestAccount::new_with_token(
            NFT_STORAGE_ACCOUNT_ID,
            NFT_TOKEN_ID,
            MIN_NFT_TOKEN_ID as u64,
        ),
    ];

    let nft_content_hash = H256::random();

    // Mint NFT.
    let mint_nft_op = MintNFTOp {
        tx: accounts[0]
            .zksync_account
            .sign_mint_nft(
                TokenId(0),
                "",
                nft_content_hash,
                BigUint::from(10u64),
                &accounts[1].account.address,
                None,
                true,
            )
            .0,
        creator_account_id: accounts[0].id,
        recipient_account_id: accounts[1].id,
    };
    let mint_nft_input =
        SigDataInput::from_mint_nft_op(&mint_nft_op).expect("SigDataInput creation failed");

    // FullExit NFT.
    let full_exit_sucess = true;
    let full_exit_op = FullExitOp {
        priority_op: FullExit {
            account_id: accounts[1].id,
            eth_address: accounts[1].account.address,
            token: TokenId(MIN_NFT_TOKEN_ID),
        },
        withdraw_amount: Some(BigUint::from(1u32).into()),
        creator_account_id: Some(mint_nft_op.creator_account_id),
        creator_address: Some(accounts[0].account.address),
        serial_id: Some(0),
        content_hash: Some(mint_nft_op.tx.content_hash),
    };

    // Initialize Plasma and WitnessBuilder.
    let (mut plasma_state, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);
    let mut witness_accum =
        WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, BlockNumber(1), 0);

    // Fees to be collected.
    let mut fees = vec![];

    // Apply MintNFT op.
    let fee = <ZkSyncState as TxHandler<MintNFT>>::apply_op(&mut plasma_state, &mint_nft_op)
        .expect("Operation failed")
        .0
        .unwrap();
    fees.push(fee);

    let witness = MintNFTWitness::apply_tx(&mut witness_accum.account_tree, &mint_nft_op);
    let circuit_operations = witness.calculate_operations(mint_nft_input);
    let pub_data_from_witness = witness.get_pubdata();
    let offset_commitment = witness.get_offset_commitment_data();

    witness_accum.add_operation_with_pubdata(
        circuit_operations,
        pub_data_from_witness,
        offset_commitment,
    );

    // Apply FullExit NFT op.
    <ZkSyncState as TxHandler<FullExit>>::apply_op(&mut plasma_state, &full_exit_op)
        .expect("Operation failed");

    let witness = FullExitWitness::apply_tx(
        &mut witness_accum.account_tree,
        &(full_exit_op, full_exit_sucess),
    );
    let circuit_operations = witness.calculate_operations(());
    let pub_data_from_witness = witness.get_pubdata();
    let offset_commitment = witness.get_offset_commitment_data();

    witness_accum.add_operation_with_pubdata(
        circuit_operations,
        pub_data_from_witness,
        offset_commitment,
    );

    // Collect fees.
    plasma_state.collect_fee(&fees, FEE_ACCOUNT_ID);
    witness_accum.collect_fees(&fees);
    witness_accum.calculate_pubdata_commitment();

    // Check that root hashes match
    assert_eq!(
        plasma_state.root_hash(),
        witness_accum
            .root_after_fees
            .expect("witness accum after root hash empty"),
        "root hash in state keeper and witness generation code mismatch"
    );

    witness_accum.into_circuit_instance()
}

/// Basic check for `FullExit` of NFT token.
#[test]
#[ignore]
fn test_full_exit_nft_success() {
    let circuit = apply_nft_mint_and_full_exit_nft_operations();

    // Verify that there are no unsatisfied constraints
    check_circuit(circuit);
}

/// Checks that executing a FullExit of NFT with
/// incorrect content_hash results in an error.
#[test]
#[ignore]
fn test_full_exit_nft_with_incorrect_content_hash() {
    const ERR_MSG: &str = "chunk number 6/execute_op/op_valid is true/enforce equal to one";

    let mut circuit = apply_nft_mint_and_full_exit_nft_operations();
    for operation_id in MintNFTOp::CHUNKS..MintNFTOp::CHUNKS + FullExitOp::CHUNKS {
        circuit.operations[operation_id].args.special_content_hash =
            vec![Some(Fr::zero()); CONTENT_HASH_WIDTH];
    }

    let result = check_circuit_non_panicking(circuit);
    match result {
        Ok(_) => panic!(
            "Operation did not err, but was expected to err with message '{}'",
            ERR_MSG,
        ),
        Err(error_msg) => {
            assert!(
                error_msg.contains(ERR_MSG),
                "Code erred with unexpected message. \
                 Provided message: '{}', but expected '{}'.",
                error_msg,
                ERR_MSG,
            );
        }
    }
}

/// Checks that executing a FullExit of NFT with
/// incorrect creator_address results in an error.
#[test]
#[ignore]
fn test_full_exit_nft_with_incorrect_creator_address() {
    const ERR_MSG: &str = "chunk number 7/execute_op/op_valid is true/enforce equal to one";

    let mut circuit = apply_nft_mint_and_full_exit_nft_operations();
    let incorrect_creator_address = incorrect_fr();
    for operation_id in MintNFTOp::CHUNKS..MintNFTOp::CHUNKS + FullExitOp::CHUNKS {
        circuit.operations[operation_id].args.special_eth_addresses[0] =
            Some(incorrect_creator_address);
    }

    let result = check_circuit_non_panicking(circuit);
    match result {
        Ok(_) => panic!(
            "Operation did not err, but was expected to err with message '{}'",
            ERR_MSG,
        ),
        Err(error_msg) => {
            assert!(
                error_msg.contains(ERR_MSG),
                "Code erred with unexpected message. \
                 Provided message: '{}', but expected '{}'.",
                error_msg,
                ERR_MSG,
            );
        }
    }
}

#[test]
#[ignore]
fn test_full_exit_failure_no_account_in_tree() {
    // Input data.
    let accounts = &[];
    let account = WitnessTestAccount::new_empty(AccountId(1)); // Will not be included into ZkSyncState
    let full_exit_op = FullExitOp {
        priority_op: FullExit {
            account_id: account.id,
            eth_address: account.account.address,
            token: TokenId(0),
        },
        withdraw_amount: None,
        creator_account_id: None,
        creator_address: None,
        serial_id: None,
        content_hash: None,
    };
    let success = false;

    generic_test_scenario::<FullExitWitness<Bn256>, _>(
        accounts,
        (full_exit_op, success),
        (),
        |plasma_state, op| {
            <ZkSyncState as TxHandler<FullExit>>::apply_op(plasma_state, &op.0)
                .expect("FullExit failed");
            vec![]
        },
    );
}

#[test]
#[ignore]
fn test_full_exit_initialted_from_wrong_account_owner() {
    // Input data.
    let accounts = vec![WitnessTestAccount::new(AccountId(1), 10)];
    let invalid_account = WitnessTestAccount::new(AccountId(2), 10);
    let account = &accounts[0];
    let invalid_account_eth_address = invalid_account.account.address;
    assert!(invalid_account_eth_address != account.account.address);
    let full_exit_op = FullExitOp {
        priority_op: FullExit {
            account_id: account.id,
            eth_address: invalid_account_eth_address,
            token: TokenId(0),
        },
        withdraw_amount: Some(BigUint::from(0u32).into()),
        creator_account_id: None,
        creator_address: None,
        serial_id: None,
        content_hash: None,
    };
    let success = false;

    generic_test_scenario::<FullExitWitness<Bn256>, _>(
        &accounts,
        (full_exit_op, success),
        (),
        |_plasma_state, _op| {
            // this operation should change nothing
            vec![]
        },
    );
}

/// Checks that executing a withdraw operation with incorrect
/// withdraw amount results in an error.
#[test]
#[ignore]
fn test_incorrect_full_exit_withdraw_amount() {
    // Test vector of (initial_balance, withdraw_amount, success).
    // Transactions are expected to fail with any value of provided `success` flag.
    let test_vector = vec![
        (10u64, 10000u64, true), // Withdraw too big and `success` set to true
        (0, 1, true),            // Withdraw from 0 balance and `success` set to true
        (10, 10000, false),      // Withdraw too big and `success` set to false
        (0, 1, false),           // Withdraw from 0 balance and `success` set to false
    ];

    // Operation is incorrect, since we try to withdraw more funds than account has.
    const ERR_MSG: &str = "op_valid is true/enforce equal to one";

    for (initial_balance, withdraw_amount, success) in test_vector {
        // Input data.
        let accounts = vec![WitnessTestAccount::new(AccountId(1), initial_balance)];
        let account = &accounts[0];
        let full_exit_op = FullExitOp {
            priority_op: FullExit {
                account_id: account.id,
                eth_address: account.account.address,
                token: TokenId(0),
            },
            withdraw_amount: Some(BigUint::from(withdraw_amount).into()),
            creator_account_id: None,
            creator_address: None,
            serial_id: None,
            content_hash: None,
        };

        #[allow(clippy::redundant_closure)]
        incorrect_op_test_scenario::<FullExitWitness<Bn256>, _, _>(
            &accounts,
            (full_exit_op, success),
            (),
            ERR_MSG,
            || vec![],
            |_| {},
        );
    }
}
