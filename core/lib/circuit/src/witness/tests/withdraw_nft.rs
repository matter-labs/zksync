// External deps
use num::BigUint;
use zksync_crypto::franklin_crypto::bellman::pairing::{
    bn256::{Bn256, Fr},
    ff::Field,
};
use zksync_crypto::params::{
    CONTENT_HASH_WIDTH, MIN_NFT_TOKEN_ID, NFT_STORAGE_ACCOUNT_ID, NFT_TOKEN_ID,
};
// Workspace deps
use zksync_state::{handler::TxHandler, state::ZkSyncState};
use zksync_types::{
    operations::{MintNFTOp, WithdrawNFTOp},
    AccountId, BlockNumber, MintNFT, TokenId, WithdrawNFT, H256,
};
// Local deps
use crate::{
    circuit::ZkSyncCircuit,
    witness::{
        tests::test_utils::{
            check_circuit, check_circuit_non_panicking, WitnessTestAccount, ZkSyncStateGenerator,
            FEE_ACCOUNT_ID,
        },
        utils::{SigDataInput, WitnessBuilder},
        MintNFTWitness, WithdrawNFTWitness, Witness,
    },
};

fn apply_nft_mint_and_withdraw_operations() -> ZkSyncCircuit<'static, Bn256> {
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

    // Withdraw NFT.
    let withdraw_nft_op = WithdrawNFTOp {
        tx: accounts[1]
            .zksync_account
            .sign_withdraw_nft(
                TokenId(MIN_NFT_TOKEN_ID),
                TokenId(0),
                "",
                BigUint::from(10u64),
                &accounts[1].account.address,
                None,
                true,
                Default::default(),
            )
            .0,
        creator_id: accounts[0].id,
        creator_address: accounts[0].account.address,
        content_hash: nft_content_hash,
        serial_id: 0,
    };
    let withdraw_nft_input =
        SigDataInput::from_withdraw_nft_op(&withdraw_nft_op).expect("SigDataInput creation failed");

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

    // Apply WithdrawNFT op.
    let fee =
        <ZkSyncState as TxHandler<WithdrawNFT>>::apply_op(&mut plasma_state, &withdraw_nft_op)
            .expect("Operation failed")
            .0
            .unwrap();
    fees.push(fee);

    let witness = WithdrawNFTWitness::apply_tx(&mut witness_accum.account_tree, &withdraw_nft_op);
    let circuit_operations = witness.calculate_operations(withdraw_nft_input);
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

/// Basic check for execution of `WithdrawNFT` operation in circuit.
/// Here we create two accounts and perform mintNFT and withdrawNFT operations.
#[test]
#[ignore]
fn test_mint_and_withdraw_nft() {
    let circuit = apply_nft_mint_and_withdraw_operations();

    // Verify that there are no unsatisfied constraints
    check_circuit(circuit);
}

/// Checks that executing a withdrawNFT operation with
/// incorrect content_hash results in an error.
#[test]
#[ignore]
fn test_withdraw_nft_with_incorrect_content_hash() {
    const ERR_MSG: &str = "chunk number 7/execute_op/op_valid is true/enforce equal to one";

    let mut circuit = apply_nft_mint_and_withdraw_operations();
    for operation_id in MintNFTOp::CHUNKS..MintNFTOp::CHUNKS + WithdrawNFTOp::CHUNKS {
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

/// Checks that executing a withdrawNFT operation with
/// incorrect serial_id results in an error.
#[test]
#[ignore]
fn test_withdraw_nft_with_incorrect_serial_id() {
    const ERR_MSG: &str = "chunk number 7/execute_op/op_valid is true/enforce equal to one";

    let mut circuit = apply_nft_mint_and_withdraw_operations();
    for operation_id in MintNFTOp::CHUNKS..MintNFTOp::CHUNKS + WithdrawNFTOp::CHUNKS {
        circuit.operations[operation_id].args.special_serial_id = Some(Fr::one());
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

/// Checks that executing a withdrawNFT operation with
/// zero balance results in an error.
#[test]
#[ignore]
fn test_withdraw_nft_with_zero_balance() {
    const ERR_MSG: &str = "chunk number 6/execute_op/op_valid is true/enforce equal to one";

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
                BigUint::from(0u64),          // zero fee in this test
                &accounts[0].account.address, // minted not to accounts[1]
                None,
                true,
            )
            .0,
        creator_account_id: accounts[0].id,
        recipient_account_id: accounts[0].id, // minted not to accounts[1]
    };
    let mint_nft_input =
        SigDataInput::from_mint_nft_op(&mint_nft_op).expect("SigDataInput creation failed");

    // Withdraw NFT.
    let withdraw_nft_op = WithdrawNFTOp {
        tx: accounts[1]
            .zksync_account
            .sign_withdraw_nft(
                TokenId(MIN_NFT_TOKEN_ID),
                TokenId(0),
                "",
                BigUint::from(0u64), // zero fee in this test
                &accounts[1].account.address,
                None,
                true,
                Default::default(),
            )
            .0,
        creator_id: accounts[0].id,
        creator_address: accounts[0].account.address,
        content_hash: nft_content_hash,
        serial_id: 0,
    };
    let withdraw_nft_input =
        SigDataInput::from_withdraw_nft_op(&withdraw_nft_op).expect("SigDataInput creation failed");

    // Initialize Plasma and WitnessBuilder.
    let (mut _plasma_state, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);
    let mut witness_accum =
        WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, BlockNumber(1), 0);

    let witness = MintNFTWitness::apply_tx(&mut witness_accum.account_tree, &mint_nft_op);
    let circuit_operations = witness.calculate_operations(mint_nft_input);
    let pub_data_from_witness = witness.get_pubdata();
    let offset_commitment = witness.get_offset_commitment_data();

    witness_accum.add_operation_with_pubdata(
        circuit_operations,
        pub_data_from_witness,
        offset_commitment,
    );

    let witness = WithdrawNFTWitness::apply_tx(&mut witness_accum.account_tree, &withdraw_nft_op);
    let circuit_operations = witness.calculate_operations(withdraw_nft_input);
    let pub_data_from_witness = witness.get_pubdata();
    let offset_commitment = witness.get_offset_commitment_data();

    witness_accum.add_operation_with_pubdata(
        circuit_operations,
        pub_data_from_witness,
        offset_commitment,
    );

    // Collect fees.
    witness_accum.collect_fees(&[]);
    witness_accum.calculate_pubdata_commitment();

    let result = check_circuit_non_panicking(witness_accum.into_circuit_instance());
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

/// Checks that corrupted signature data leads to unsatisfied constraints in circuit.
#[test]
#[ignore]
fn test_withdraw_nft_corrupted_ops_input() {
    const ERR_MSG: &str = "chunk number 5/execute_op/op_valid is true/enforce equal to one";

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
                BigUint::from(0u64), // zero fee in this test
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

    // Withdraw NFT.
    let withdraw_nft_op = WithdrawNFTOp {
        tx: accounts[1]
            .zksync_account
            .sign_withdraw_nft(
                TokenId(MIN_NFT_TOKEN_ID),
                TokenId(0),
                "",
                BigUint::from(0u64), // zero fee in this test
                &accounts[1].account.address,
                None,
                true,
                Default::default(),
            )
            .0,
        creator_id: accounts[0].id,
        creator_address: accounts[0].account.address,
        content_hash: nft_content_hash,
        serial_id: 0,
    };
    let withdraw_nft_input =
        SigDataInput::from_withdraw_nft_op(&withdraw_nft_op).expect("SigDataInput creation failed");

    // Test vector with values corrupted one by one.
    let test_vector = withdraw_nft_input.corrupted_variations();

    for withdraw_nft_corrupted_input in test_vector {
        // Initialize Plasma and WitnessBuilder.
        let (mut _plasma_state, mut circuit_account_tree) =
            ZkSyncStateGenerator::generate(&accounts);
        let mut witness_accum =
            WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, BlockNumber(1), 0);

        let witness = MintNFTWitness::apply_tx(&mut witness_accum.account_tree, &mint_nft_op);
        let circuit_operations = witness.calculate_operations(mint_nft_input.clone());
        let pub_data_from_witness = witness.get_pubdata();
        let offset_commitment = witness.get_offset_commitment_data();

        witness_accum.add_operation_with_pubdata(
            circuit_operations,
            pub_data_from_witness,
            offset_commitment,
        );

        let witness =
            WithdrawNFTWitness::apply_tx(&mut witness_accum.account_tree, &withdraw_nft_op);
        let circuit_operations = witness.calculate_operations(withdraw_nft_corrupted_input);
        let pub_data_from_witness = witness.get_pubdata();
        let offset_commitment = witness.get_offset_commitment_data();

        witness_accum.add_operation_with_pubdata(
            circuit_operations,
            pub_data_from_witness,
            offset_commitment,
        );

        // Collect fees.
        witness_accum.collect_fees(&[]);
        witness_accum.calculate_pubdata_commitment();

        let result = check_circuit_non_panicking(witness_accum.into_circuit_instance());
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
}
