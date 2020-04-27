// External deps
use bigdecimal::BigDecimal;
// Workspace deps
use models::node::{operations::WithdrawOp, Account, Address};
use testkit::zksync_account::ZksyncAccount;
// Local deps
use crate::witness::{
    tests::test_utils::{check_circuit, test_genesis_plasma_state},
    utils::{prepare_sig_data, WitnessBuilder},
    withdraw::{apply_withdraw_tx, calculate_withdraw_operations_from_witness},
};

#[test]
#[ignore]
fn test_withdraw() {
    let zksync_account = ZksyncAccount::rand();
    let account_id = 1;
    let account_address = zksync_account.address;
    let account = {
        let mut account = Account::default_with_address(&account_address);
        account.add_balance(0, &BigDecimal::from(10));
        account.pub_key_hash = zksync_account.pubkey_hash.clone();
        account
    };

    let (mut plasma_state, mut circuit_account_tree) =
        test_genesis_plasma_state(vec![(account_id, account)]);
    let fee_account_id = 0;
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

    let withdraw_op = WithdrawOp {
        tx: zksync_account
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
        account_id,
    };

    println!("node root hash before op: {:?}", plasma_state.root_hash());
    let (fee, _) = plasma_state
        .apply_withdraw_op(&withdraw_op)
        .expect("transfer should be success");
    println!("node root hash after op: {:?}", plasma_state.root_hash());
    plasma_state.collect_fee(&[fee.clone()], witness_accum.fee_account_id);
    println!("node root hash after fee: {:?}", plasma_state.root_hash());
    println!(
        "node withdraw tx bytes: {}",
        hex::encode(&withdraw_op.tx.get_bytes())
    );

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
