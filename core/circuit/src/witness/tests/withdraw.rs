// External deps
use bigdecimal::BigDecimal;
// Workspace deps
use models::node::{operations::WithdrawOp, Address};
// Local deps
use crate::witness::{
    tests::test_utils::{check_circuit, PlasmaStateGenerator, WitnessTestAccount, FEE_ACCOUNT_ID},
    utils::{SigDataInput, WitnessBuilder},
    withdraw::WithdrawWitness,
};

#[test]
#[ignore]
fn test_withdraw() {
    // Input data.
    let accounts = vec![WitnessTestAccount::new(1, 10)];
    let account = &accounts[0];
    let withdraw_op = WithdrawOp {
        tx: account
            .zksync_account
            .sign_withdraw(
                0,
                "",
                BigDecimal::from(7),
                BigDecimal::from(3),
                &Address::zero(),
                None,
                true,
            )
            .0,
        account_id: account.id,
    };

    // Additional data required for performing the operation.
    let sign_packed = withdraw_op
        .tx
        .signature
        .signature
        .serialize_packed()
        .expect("signature serialize");
    let input = SigDataInput::new(
        &sign_packed,
        &withdraw_op.tx.get_bytes(),
        &withdraw_op.tx.signature.pub_key,
    )
    .expect("prepare signature data");

    // Initialize Plasma and WitnessBuilder.
    let (mut plasma_state, mut circuit_account_tree) = PlasmaStateGenerator::generate(&accounts);
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, 1);

    // Apply op on plasma
    let (fee, _) = plasma_state
        .apply_withdraw_op(&withdraw_op)
        .expect("transfer should be success");
    plasma_state.collect_fee(&[fee.clone()], witness_accum.fee_account_id);

    // Apply op on circuit
    let withdraw_witness = WithdrawWitness::apply_tx(&mut witness_accum.account_tree, &withdraw_op);
    let withdraw_operations = withdraw_witness.calculate_operations(input);
    let pub_data_from_witness = withdraw_witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(withdraw_operations, pub_data_from_witness);
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
