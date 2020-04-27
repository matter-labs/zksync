// External deps
use bigdecimal::BigDecimal;
// Workspace deps
use models::node::operations::TransferToNewOp;
// Local deps
use crate::witness::{
    tests::test_utils::{check_circuit, PlasmaStateGenerator, WitnessTestAccount},
    transfer_to_new::{
        apply_transfer_to_new_tx, calculate_transfer_to_new_operations_from_witness,
    },
    utils::{prepare_sig_data, WitnessBuilder},
};

#[test]
#[ignore]
fn test_transfer_to_new_success() {
    let account_from = WitnessTestAccount::new(1, 10);
    let account_to = WitnessTestAccount::new_empty(2); // Will not be included into state.
    let (mut plasma_state, mut circuit_account_tree) =
        PlasmaStateGenerator::from_single(&account_from);

    let fee_account_id = 0;
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

    let transfer_op = TransferToNewOp {
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

    let (fee, _) = plasma_state
        .apply_transfer_to_new_op(&transfer_op)
        .expect("transfer should be success");
    plasma_state.collect_fee(&[fee.clone()], witness_accum.fee_account_id);

    let transfer_witness = apply_transfer_to_new_tx(&mut witness_accum.account_tree, &transfer_op);
    let sign_packed = transfer_op
        .tx
        .signature
        .signature
        .serialize_packed()
        .expect("signature serialize");
    let (first_sig_msg, second_sig_msg, third_sig_msg, signature_data, signer_packed_key_bits) =
        prepare_sig_data(
            &sign_packed,
            &transfer_op.tx.get_bytes(),
            &transfer_op.tx.signature.pub_key,
        )
        .expect("prepare signature data");
    let transfer_operations = calculate_transfer_to_new_operations_from_witness(
        &transfer_witness,
        &first_sig_msg,
        &second_sig_msg,
        &third_sig_msg,
        &signature_data,
        &signer_packed_key_bits,
    );
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
