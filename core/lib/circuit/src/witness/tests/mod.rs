//! Test suite for witness generation.
//! This module structure can be divided in the following sections:
//!
//! - Operation witness tests: tests for the operations, e.g. `DepositOp`,
//!   are placed in the corresponding modules.
//! - Low-level tests for circuit generation algorithm are placed in the `noop` module.
//! - Generic tests for the combinations of different operations are placed in this module.

// External deps
use num::BigUint;
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
// Workspace deps
use zksync_state::{
    handler::TxHandler,
    state::{TransferOutcome, ZkSyncState},
};
use zksync_types::{
    operations::{DepositOp, FullExitOp, TransferOp, TransferToNewOp, WithdrawOp},
    Address, Deposit, FullExit, Transfer, Withdraw,
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
        DepositWitness, FullExitWitness, TransferToNewWitness, TransferWitness, WithdrawWitness,
        Witness,
    },
};

mod change_pubkey_offchain;
mod deposit;
mod forced_exit;
mod full_exit;
mod noop;
pub(crate) mod test_utils;
mod transfer;
mod transfer_to_new;
mod withdraw;

/// Executes the following operations:
///
/// - Deposit several types of token on the account.
/// - Transfer some funds to different accounts, both existing and new.
/// - Change the public key of account.
/// - Withdraw some funds.
///
/// Returns the resulting `WitnessBuilder` and the hash obtained
/// from `ZkSyncState` for further correctness checks.
fn apply_many_ops() -> ZkSyncCircuit<'static, Bn256> {
    const ETH_TOKEN: u16 = 0;
    const NNM_TOKEN: u16 = 2;

    // Create two accounts: we will perform all the operations with the first one,
    // while the second one will be used as "target" account for transfers.
    let accounts = vec![
        WitnessTestAccount::new_empty(1),
        WitnessTestAccount::new_empty(2),
    ];
    let (account, account_to) = (&accounts[0], &accounts[1]);

    // Deposit two types of tokens on the account.
    let deposit_data = [
        (ETH_TOKEN, 1000u32), // 1000 of ETH
        (NNM_TOKEN, 2000u32), // 2000 of token with ID 2
    ];
    let deposit_ops = deposit_data
        .iter()
        .map(|(token_id, token_amount)| DepositOp {
            priority_op: Deposit {
                from: account.account.address,
                token: *token_id,
                amount: BigUint::from(*token_amount),
                to: account.account.address,
            },
            account_id: account.id,
        });

    // Transfer ETH to an existing account.
    let transfer_op = TransferOp {
        tx: account
            .zksync_account
            .sign_transfer(
                ETH_TOKEN,
                "",
                BigUint::from(97u32),
                BigUint::from(3u32),
                &account_to.account.address,
                None,
                true,
            )
            .0,
        from: account.id,
        to: account_to.id,
    };
    let transfer_input =
        SigDataInput::from_transfer_op(&transfer_op).expect("SigDataInput creation failed");

    // Transfer token to a new account.
    let new_account = WitnessTestAccount::new_empty(3);
    let transfer_to_new_op = TransferToNewOp {
        tx: account
            .zksync_account
            .sign_transfer(
                NNM_TOKEN,
                "",
                BigUint::from(1900u32),
                BigUint::from(90u32),
                &account_to.account.address,
                None,
                true,
            )
            .0,
        from: account.id,
        to: new_account.id,
    };
    let transfer_to_new_input = SigDataInput::from_transfer_to_new_op(&transfer_to_new_op)
        .expect("SigDataInput creation failed");

    // Withdraw token from account.
    // We've transferred 1990 tokens above, so we have 10 left.
    let withdraw_op = WithdrawOp {
        tx: account
            .zksync_account
            .sign_withdraw(
                NNM_TOKEN,
                "",
                BigUint::from(5u32),
                BigUint::from(5u32),
                &Address::zero(),
                None,
                true,
            )
            .0,
        account_id: account.id,
    };
    let withdraw_input =
        SigDataInput::from_withdraw_op(&withdraw_op).expect("SigDataInput creation failed");

    // Perform full exit.
    // We've transferred 100 tokens above, so we have 900 left.
    let full_exit_op = FullExitOp {
        priority_op: FullExit {
            account_id: account.id,
            eth_address: account.account.address,
            token: 0,
        },
        withdraw_amount: Some(BigUint::from(900u32).into()),
    };
    let full_exit_success = true;

    // Initialize Plasma and WitnessBuilder.
    let (mut plasma_state, mut circuit_account_tree) = ZkSyncStateGenerator::generate(&accounts);
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, 1);

    // Fees to be collected.
    let mut fees = vec![];

    // Apply deposit ops.
    for deposit_op in deposit_ops {
        <ZkSyncState as TxHandler<Deposit>>::apply_op(&mut plasma_state, &deposit_op)
            .expect("Deposit failed");

        let witness = DepositWitness::apply_tx(&mut witness_accum.account_tree, &deposit_op);
        let circuit_operations = witness.calculate_operations(());
        let pub_data_from_witness = witness.get_pubdata();

        witness_accum.add_operation_with_pubdata(circuit_operations, pub_data_from_witness);
    }

    // Apply transfer op.
    let raw_op = TransferOutcome::Transfer(transfer_op.clone());
    let fee = <ZkSyncState as TxHandler<Transfer>>::apply_op(&mut plasma_state, &raw_op)
        .expect("Operation failed")
        .0
        .unwrap();
    fees.push(fee);

    let witness = TransferWitness::apply_tx(&mut witness_accum.account_tree, &transfer_op);
    let circuit_operations = witness.calculate_operations(transfer_input);
    let pub_data_from_witness = witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(circuit_operations, pub_data_from_witness);

    // Apply transfer to new op.
    let raw_op = TransferOutcome::TransferToNew(transfer_to_new_op.clone());
    let fee = <ZkSyncState as TxHandler<Transfer>>::apply_op(&mut plasma_state, &raw_op)
        .expect("Operation failed")
        .0
        .unwrap();
    fees.push(fee);

    let witness =
        TransferToNewWitness::apply_tx(&mut witness_accum.account_tree, &transfer_to_new_op);
    let circuit_operations = witness.calculate_operations(transfer_to_new_input);
    let pub_data_from_witness = witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(circuit_operations, pub_data_from_witness);

    // Apply withdraw op.
    let fee = <ZkSyncState as TxHandler<Withdraw>>::apply_op(&mut plasma_state, &withdraw_op)
        .expect("Operation failed")
        .0
        .unwrap();
    fees.push(fee);

    let witness = WithdrawWitness::apply_tx(&mut witness_accum.account_tree, &withdraw_op);
    let circuit_operations = witness.calculate_operations(withdraw_input);
    let pub_data_from_witness = witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(circuit_operations, pub_data_from_witness);

    // Apply full exit op.

    <ZkSyncState as TxHandler<FullExit>>::apply_op(&mut plasma_state, &full_exit_op)
        .expect("Operation failed");

    let witness = FullExitWitness::apply_tx(
        &mut witness_accum.account_tree,
        &(full_exit_op, full_exit_success),
    );
    let circuit_operations = witness.calculate_operations(());
    let pub_data_from_witness = witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(circuit_operations, pub_data_from_witness);

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

/// Composite test combines all the witness types applied together within one block:
/// - Deposit several types of token on the account.
/// - Transfer some funds to different accounts, both existing and new.
/// - Change the public key of account.
/// - Withdraw some funds.
/// - Perform full exit for an account.
/// - Check the root hash and circuit constraints.
///
/// All the actions are performed within one block.
#[test]
#[ignore]
fn composite_test() {
    // Perform some operations
    let circuit = apply_many_ops();

    // Verify that there are no unsatisfied constraints
    check_circuit(circuit);
}

/// Checks that corrupted list of operations in block leads to predictable errors.
/// Check for chunk in the end of the operations list.
#[test]
#[ignore]
fn corrupted_last_operation() {
    // Perform some operations
    let mut circuit = apply_many_ops();

    // Try to cut off an operation at end.
    circuit.operations.pop();

    // As we removed the last operation, the last chunk of the block is no longer the last chunk of
    // the corresponding transaction.
    // See `circuit.rs` for details.
    let expected_msg =
        "ensure last chunk of the block is a last chunk of corresponding transaction";

    let error = check_circuit_non_panicking(circuit)
        .expect_err("Corrupted operations list should lead to an error");

    assert!(
        error.contains(expected_msg),
        "corrupted_operations: Got error message '{}', but expected '{}'",
        error,
        expected_msg
    );
}

/// Checks that corrupted list of operations in block leads to predictable errors.
/// Check for chunk in the beginning of the operations list.
#[test]
#[ignore]
fn corrupted_first_operation() {
    // Perform some operations
    let mut circuit = apply_many_ops();

    // Now try to cut off an operation at the beginning.
    circuit.operations.remove(0);

    // We corrupted the very first chunk, so it should be reported.
    // See `circuit.rs` for details.
    let expected_msg = "chunk number 0/verify_correct_chunking/correct_sequence";

    let error = check_circuit_non_panicking(circuit)
        .expect_err("Corrupted operations list should lead to an error");

    assert!(
        error.contains(expected_msg),
        "corrupted_operations: Got error message '{}', but expected '{}'",
        error,
        expected_msg
    );
}

/// Checks that corrupted list of operations in block leads to predictable errors.
/// Check for chunk in the middle of the operations list.
#[test]
#[ignore]
fn corrupted_intermediate_operation() {
    // Perform some operations
    let mut circuit = apply_many_ops();

    // Now replace the operation in the middle with incorrect operation.
    let corrupted_op_chunk = circuit.operations.len() / 2;
    circuit.operations[corrupted_op_chunk] = circuit.operations[0].clone();

    // Create an error message with the exact chunk number.
    // See `circuit.rs` for details.
    let expected_msg = format!(
        "chunk number {}/verify_correct_chunking/correct_sequence",
        corrupted_op_chunk
    );

    let error = check_circuit_non_panicking(circuit)
        .expect_err("Corrupted operations list should lead to an error");

    assert!(
        error.contains(&expected_msg),
        "corrupted_operations: Got error message '{}', but expected '{}'",
        error,
        expected_msg
    );
}

/// Checks that corrupted validator merkle proof in block leads to predictable errors.
/// Check for chunk in the end of the operations list.
#[test]
#[ignore]
fn corrupted_validator_audit_path() {
    // Perform some operations
    let mut circuit = apply_many_ops();

    // Corrupt merkle proof.
    circuit.validator_audit_path[0] = Some(Default::default());

    // Corrupted proof will lead to incorrect root hash.
    // See `circuit.rs` for details.
    let expected_msg = "root before applying fees is correct";

    let error = check_circuit_non_panicking(circuit)
        .expect_err("Corrupted operations list should lead to an error");

    assert!(
        error.contains(expected_msg),
        "corrupted_operations: Got error message '{}', but expected '{}'",
        error,
        expected_msg
    );
}
