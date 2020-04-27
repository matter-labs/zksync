// External deps
use bigdecimal::BigDecimal;
// Workspace deps
use models::node::{operations::TransferOp, Account};
use testkit::zksync_account::ZksyncAccount;
// Local deps
use crate::witness::{
    tests::test_utils::{check_circuit, test_genesis_plasma_state},
    transfer::{apply_transfer_tx, calculate_transfer_operations_from_witness},
    utils::{prepare_sig_data, WitnessBuilder},
};

#[test]
#[ignore]
fn test_transfer_success() {
    let from_zksync_account = ZksyncAccount::rand();
    let from_account_id = 1;
    let from_account_address = from_zksync_account.address;
    let from_account = {
        let mut account = Account::default_with_address(&from_account_address);
        account.add_balance(0, &BigDecimal::from(10));
        account.pub_key_hash = from_zksync_account.pubkey_hash.clone();
        account
    };

    let to_account_id = 2;
    let to_account_address = "2222222222222222222222222222222222222222".parse().unwrap();
    let to_account = Account::default_with_address(&to_account_address);

    let (mut plasma_state, mut circuit_account_tree) = test_genesis_plasma_state(vec![
        (from_account_id, from_account),
        (to_account_id, to_account),
    ]);
    let fee_account_id = 0;
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

    let transfer_op = TransferOp {
        tx: from_zksync_account
            .sign_transfer(
                0,
                "",
                BigDecimal::from(7),
                BigDecimal::from(3),
                &to_account_address,
                None,
                true,
            )
            .0,
        from: from_account_id,
        to: to_account_id,
    };

    let (fee, _) = plasma_state
        .apply_transfer_op(&transfer_op)
        .expect("transfer should be success");
    plasma_state.collect_fee(&[fee.clone()], witness_accum.fee_account_id);

    let transfer_witness = apply_transfer_tx(&mut witness_accum.account_tree, &transfer_op);
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
    let transfer_operations = calculate_transfer_operations_from_witness(
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

#[test]
#[ignore]
fn test_transfer_to_self() {
    let from_zksync_account = ZksyncAccount::rand();
    let from_account_id = 1;
    let from_account_address = from_zksync_account.address;
    let from_account = {
        let mut account = Account::default_with_address(&from_account_address);
        account.add_balance(0, &BigDecimal::from(10));
        account.pub_key_hash = from_zksync_account.pubkey_hash.clone();
        account
    };

    let (mut plasma_state, mut circuit_account_tree) =
        test_genesis_plasma_state(vec![(from_account_id, from_account)]);

    let fee_account_id = 0;
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

    let transfer_op = TransferOp {
        tx: from_zksync_account
            .sign_transfer(
                0,
                "",
                BigDecimal::from(7),
                BigDecimal::from(3),
                &from_account_address,
                None,
                true,
            )
            .0,
        from: from_account_id,
        to: from_account_id,
    };

    let (fee, _) = plasma_state
        .apply_transfer_op(&transfer_op)
        .expect("transfer should be success");
    plasma_state.collect_fee(&[fee.clone()], witness_accum.fee_account_id);

    let transfer_witness = apply_transfer_tx(&mut witness_accum.account_tree, &transfer_op);
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
    let transfer_operations = calculate_transfer_operations_from_witness(
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
