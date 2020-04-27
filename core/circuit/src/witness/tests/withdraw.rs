// External deps
use bigdecimal::BigDecimal;
// Workspace deps
use models::node::{operations::WithdrawOp, Address};
// Local deps
use crate::witness::{
    tests::test_utils::{check_circuit, PlasmaStateGenerator, WitnessTestAccount},
    utils::{prepare_sig_data, WitnessBuilder},
    withdraw::{apply_withdraw_tx, calculate_withdraw_operations_from_witness},
};

#[test]
#[ignore]
fn test_withdraw() {
    let account = WitnessTestAccount::new(1, 10);
    let (mut plasma_state, mut circuit_account_tree) = PlasmaStateGenerator::from_single(&account);

    let fee_account_id = 0;
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

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

    let (fee, _) = plasma_state
        .apply_withdraw_op(&withdraw_op)
        .expect("transfer should be success");
    plasma_state.collect_fee(&[fee.clone()], witness_accum.fee_account_id);

    let withdraw_witness = apply_withdraw_tx(&mut witness_accum.account_tree, &withdraw_op);
    let sign_packed = withdraw_op
        .tx
        .signature
        .signature
        .serialize_packed()
        .expect("signature serialize");
    let (first_sig_msg, second_sig_msg, third_sig_msg, signature_data, signer_packed_key_bits) =
        prepare_sig_data(
            &sign_packed,
            &withdraw_op.tx.get_bytes(),
            &withdraw_op.tx.signature.pub_key,
        )
        .expect("prepare signature data");
    let withdraw_operations = calculate_withdraw_operations_from_witness(
        &withdraw_witness,
        &first_sig_msg,
        &second_sig_msg,
        &third_sig_msg,
        &signature_data,
        &signer_packed_key_bits,
    );
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
