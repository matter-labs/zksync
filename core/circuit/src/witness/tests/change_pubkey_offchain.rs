// Workspace deps
use models::{node::operations::ChangePubKeyOp, primitives::pack_bits_into_bytes_in_order};
// Local deps
use crate::witness::{
    change_pubkey_offchain::ChangePubkeyOffChainWitness,
    tests::test_utils::{check_circuit, PlasmaStateGenerator, WitnessTestAccount, FEE_ACCOUNT_ID},
    utils::WitnessBuilder,
};

/// Basic check for execution of `ChangePubKeyOp` in circuit.
/// Here we generate an empty account and change its public key.
#[test]
#[ignore]
fn test_change_pubkey_offchain_success() {
    // Input data.
    let account = WitnessTestAccount::new_empty(0xc1);
    let change_pkhash_op = ChangePubKeyOp {
        tx: account
            .zksync_account
            .create_change_pubkey_tx(None, true, false),
        account_id: account.id,
    };

    // Initialize Plasma and WitnessBuilder.
    let (mut plasma_state, mut circuit_account_tree) = PlasmaStateGenerator::from_single(&account);
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, FEE_ACCOUNT_ID, 1);

    // Apply op on plasma
    plasma_state
        .apply_change_pubkey_op(&change_pkhash_op)
        .expect("Operation failed");

    // Apply op on circuit
    let change_pkhash_witness =
        ChangePubkeyOffChainWitness::apply_tx(&mut witness_accum.account_tree, &change_pkhash_op);
    let change_pkhash_operations = change_pkhash_witness.calculate_operations();
    let pub_data_from_witness = change_pkhash_witness.get_pubdata();

    // Check that pubdata observed from witness is correct
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
