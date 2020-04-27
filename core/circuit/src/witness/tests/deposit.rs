// External deps
use bigdecimal::BigDecimal;
// Workspace deps
use models::node::{operations::DepositOp, Deposit};
// Local deps
use crate::witness::{
    deposit::{apply_deposit_tx, calculate_deposit_operations_from_witness},
    tests::test_utils::{check_circuit, PlasmaStateGenerator, WitnessTestAccount},
    utils::WitnessBuilder,
};

#[test]
#[ignore]
fn test_deposit_in_empty_leaf() {
    let (mut plasma_state, mut circuit_account_tree) = PlasmaStateGenerator::generate_empty();

    let fee_account_id = 0;
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

    let empty_account_id = 1;
    let empty_account_address = [7u8; 20].into();
    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: empty_account_address,
            token: 0,
            amount: BigDecimal::from(1),
            to: empty_account_address,
        },
        account_id: empty_account_id,
    };

    println!(
        "node root hash before deposit: {:?}",
        plasma_state.root_hash()
    );
    plasma_state.apply_deposit_op(&deposit_op);
    println!(
        "node root hash after deposit: {:?}",
        plasma_state.root_hash()
    );
    println!(
        "node pub data: {}",
        hex::encode(&deposit_op.get_public_data())
    );

    let deposit_witness = apply_deposit_tx(&mut witness_accum.account_tree, &deposit_op);
    let deposit_operations = calculate_deposit_operations_from_witness(&deposit_witness);
    let pub_data_from_witness = deposit_witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(deposit_operations, pub_data_from_witness);
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

#[test]
#[ignore]
fn test_deposit_existing_account() {
    let account = WitnessTestAccount::new_empty(1);
    let (mut plasma_state, mut circuit_account_tree) = PlasmaStateGenerator::from_single(&account);

    let fee_account_id = 0;
    let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: account.account.address,
            token: 0,
            amount: BigDecimal::from(1),
            to: account.account.address,
        },
        account_id: account.id,
    };

    println!("node root hash before op: {:?}", plasma_state.root_hash());
    plasma_state.apply_deposit_op(&deposit_op);
    println!("node root hash after op: {:?}", plasma_state.root_hash());

    let deposit_witness = apply_deposit_tx(&mut witness_accum.account_tree, &deposit_op);
    let deposit_operations = calculate_deposit_operations_from_witness(&deposit_witness);
    let pub_data_from_witness = deposit_witness.get_pubdata();

    witness_accum.add_operation_with_pubdata(deposit_operations, pub_data_from_witness);
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
