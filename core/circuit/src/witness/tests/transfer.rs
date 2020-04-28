// External deps
use bigdecimal::BigDecimal;
// Workspace deps
use models::node::operations::TransferOp;
// Local deps
use crate::witness::{
    tests::test_utils::{check_circuit, PlasmaStateGenerator, WitnessTestAccount, FEE_ACCOUNT_ID},
    transfer::TransferWitness,
    utils::{SigDataInput, WitnessBuilder},
};

/// Basic check for execution of `Transfer` operation in circuit.
/// Here we create two accounts and perform a transfer between them.
#[test]
#[ignore]
fn test_transfer_success() {
    // Input data.
    let accounts = vec![
        WitnessTestAccount::new(1, 10),
        WitnessTestAccount::new_empty(2),
    ];
    let (account_from, account_to) = (&accounts[0], &accounts[1]);
    let transfer_op = TransferOp {
        tx: account_from
            .zksync_account
            .sign_transfer(
                0,
                "",
                BigDecimal::from(7),
                BigDecimal::from(3),
                &account_to.account.address,
                None,
                true,
            )
            .0,
        from: account_from.id,
        to: account_to.id,
    };

    // Additional data required for performing the operation.
    let sign_packed = transfer_op
        .tx
        .signature
        .signature
        .serialize_packed()
        .expect("signature serialize");
    let input = SigDataInput::new(
        &sign_packed,
        &transfer_op.tx.get_bytes(),
        &transfer_op.tx.signature.pub_key,
    )
    .expect("prepare signature data");

    // Initialize Plasma and WitnessBuilder.
    let (mut plasma_state, mut circuit_account_tree) = PlasmaStateGenerator::generate(&accounts);
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, 1);

    // Apply op on plasma
    let (fee, _) = plasma_state
        .apply_transfer_op(&transfer_op)
        .expect("transfer should be success");
    plasma_state.collect_fee(&[fee.clone()], witness_accum.fee_account_id);

    // Apply op on circuit
    let transfer_witness = TransferWitness::apply_tx(&mut witness_accum.account_tree, &transfer_op);
    let transfer_operations = transfer_witness.calculate_operations(input);
    let pub_data_from_witness = transfer_witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(transfer_operations, pub_data_from_witness);
    witness_accum.collect_fees(&[fee]);
    witness_accum.calculate_pubdata_commitment();

    assert_eq!(
        plasma_state.root_hash(),
        witness_accum
            .root_after_fees
            .expect("witness accum after root hash empty"),
        "root hash in state keeper and witness generation code mismatch"
    );

    check_circuit(witness_accum.into_circuit_instance());
}

/// Check for execution of `Transfer` operation with recipient same as sender in circuit.
/// Here we create one accounts and perform a transfer to self.
#[test]
#[ignore]
fn test_transfer_to_self() {
    // Input data.
    let accounts = vec![WitnessTestAccount::new(1, 10)];
    let account = &accounts[0];
    let transfer_op = TransferOp {
        tx: account
            .zksync_account
            .sign_transfer(
                0,
                "",
                BigDecimal::from(7),
                BigDecimal::from(3),
                &account.account.address,
                None,
                true,
            )
            .0,
        from: account.id,
        to: account.id,
    };

    // Additional data required for performing the operation.
    let sign_packed = transfer_op
        .tx
        .signature
        .signature
        .serialize_packed()
        .expect("signature serialize");
    let input = SigDataInput::new(
        &sign_packed,
        &transfer_op.tx.get_bytes(),
        &transfer_op.tx.signature.pub_key,
    )
    .expect("prepare signature data");

    // Initialize Plasma and WitnessBuilder.
    let (mut plasma_state, mut circuit_account_tree) = PlasmaStateGenerator::generate(&accounts);
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, 1);

    // Apply op on plasma
    let (fee, _) = plasma_state
        .apply_transfer_op(&transfer_op)
        .expect("transfer should be success");
    plasma_state.collect_fee(&[fee.clone()], witness_accum.fee_account_id);

    // Apply op on circuit
    let transfer_witness = TransferWitness::apply_tx(&mut witness_accum.account_tree, &transfer_op);
    let transfer_operations = transfer_witness.calculate_operations(input);
    let pub_data_from_witness = transfer_witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(transfer_operations, pub_data_from_witness);
    witness_accum.collect_fees(&[fee]);
    witness_accum.calculate_pubdata_commitment();

    assert_eq!(
        plasma_state.root_hash(),
        witness_accum
            .root_after_fees
            .expect("witness accum after root hash empty"),
        "root hash in state keeper and witness generation code mismatch"
    );

    check_circuit(witness_accum.into_circuit_instance());
}
