// Workspace deps
use models::{
    node::{operations::ChangePubKeyOp, Account},
    primitives::pack_bits_into_bytes_in_order,
};
use testkit::zksync_account::ZksyncAccount;
// Local deps
use crate::witness::{
    change_pubkey_offchain::{
        apply_change_pubkey_offchain_tx, calculate_change_pubkey_offchain_from_witness,
    },
    tests::test_utils::{check_circuit, test_genesis_plasma_state},
    utils::WitnessBuilder,
};

#[test]
#[ignore]
fn test_change_pubkey_offchain_success() {
    let zksync_account = ZksyncAccount::rand();
    let change_pkhash_to_account_id = 0xc1;
    let change_pkhash_to_account_address = zksync_account.address;
    let (mut plasma_state, mut circuit_account_tree) = test_genesis_plasma_state(vec![(
        change_pkhash_to_account_id,
        Account::default_with_address(&change_pkhash_to_account_address),
    )]);

    let fee_account_id = 0;
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

    let change_pkhash_op = ChangePubKeyOp {
        tx: zksync_account.create_change_pubkey_tx(None, true, false),
        account_id: change_pkhash_to_account_id,
    };

    println!("node root hash before op: {:?}", plasma_state.root_hash());
    plasma_state
        .apply_change_pubkey_op(&change_pkhash_op)
        .expect("applying op fail");
    println!("node root hash after op: {:?}", plasma_state.root_hash());
    println!(
        "node pubdata: {}",
        hex::encode(&change_pkhash_op.get_public_data())
    );

    let change_pkhash_witness =
        apply_change_pubkey_offchain_tx(&mut witness_accum.account_tree, &change_pkhash_op);
    let change_pkhash_operations =
        calculate_change_pubkey_offchain_from_witness(&change_pkhash_witness);
    let pub_data_from_witness = change_pkhash_witness.get_pubdata();

    //        println!("Change pk onchain witness: {:#?}", change_pkhash_witness);

    assert_eq!(
        hex::encode(pack_bits_into_bytes_in_order(pub_data_from_witness.clone())),
        hex::encode(change_pkhash_op.get_public_data()),
        "pubdata from witness incorrect"
    );

    witness_accum.add_operation_with_pubdata(change_pkhash_operations, pub_data_from_witness);
    witness_accum.collect_fees(&Vec::new());
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
