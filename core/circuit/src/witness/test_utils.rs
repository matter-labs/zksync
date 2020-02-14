use crate::account::AccountWitness;
use crate::circuit::FranklinCircuit;
use crate::franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use crate::franklin_crypto::bellman::pairing::ff::{Field, PrimeField};
use crate::franklin_crypto::bellman::Circuit;
use crate::franklin_crypto::circuit::test::TestConstraintSystem;
use crate::operation::Operation;
use models::circuit::account::CircuitAccount;
use models::circuit::CircuitAccountTree;
use models::merkle_tree::PedersenHasher;
use models::node::{Account, AccountId, Address, BlockNumber, Engine, Fr};
use models::primitives::big_decimal_to_u128;
use plasma::state::{CollectedFee, PlasmaState};

/// Wrapper around `CircuitAccountTree`
/// that simplifies witness generation
/// used for testing
pub struct WitnessAccumulator {
    pub account_tree: CircuitAccountTree,
    pub fee_account_id: AccountId,
    pub block_number: BlockNumber,
    pub initial_root_hash: Fr,
    pub operations: Vec<Operation<Engine>>,
    pub pubdata: Vec<bool>,
    pub root_before_fees: Option<Fr>,
    pub root_after_fees: Option<Fr>,
    pub fee_account_balances: Option<Vec<Option<Fr>>>,
    pub fee_account_witness: Option<AccountWitness<Engine>>,
    pub fee_account_audit_path: Option<Vec<Option<Fr>>>,
    pub pubdata_commitment: Option<Fr>,
}

impl WitnessAccumulator {
    pub fn new(
        account_tree: CircuitAccountTree,
        fee_account_id: AccountId,
        block_number: BlockNumber,
    ) -> WitnessAccumulator {
        let initial_root_hash = account_tree.root_hash();
        WitnessAccumulator {
            account_tree,
            fee_account_id,
            block_number,
            initial_root_hash,
            operations: Vec::new(),
            pubdata: Vec::new(),
            root_before_fees: None,
            root_after_fees: None,
            fee_account_balances: None,
            fee_account_witness: None,
            fee_account_audit_path: None,
            pubdata_commitment: None,
        }
    }

    /// Add witness generated for operation
    pub fn add_operation_with_pubdata(&mut self, ops: Vec<Operation<Engine>>, pubdata: Vec<bool>) {
        self.operations.extend(ops.into_iter());
        self.pubdata.extend(pubdata.into_iter());
    }

    /// Add noops if pubdata isn't of right size
    pub fn extend_pubdata_with_noops(
        &mut self,
        phasher: &PedersenHasher<Engine>,
        params: &AltJubjubBn256,
    ) {
        for _ in 0..models::params::block_size_chunks() - self.operations.len() {
            let (signature, first_sig_msg, second_sig_msg, third_sig_msg, _sender_x, _sender_y) =
                crate::witness::utils::generate_dummy_sig_data(&[false], &phasher, &params);
            self.operations.push(crate::witness::noop::noop_operation(
                &self.account_tree,
                self.fee_account_id,
                &first_sig_msg,
                &second_sig_msg,
                &third_sig_msg,
                &signature,
                &[Some(false); 256],
            ));
            self.pubdata.extend(vec![false; 64]);
        }
    }

    /// After operations are added, collect fees.
    pub fn collect_fees(&mut self, fees: &[CollectedFee]) {
        self.root_before_fees = Some(self.account_tree.root_hash());

        let fee_circuit_account = self
            .account_tree
            .get(self.fee_account_id)
            .expect("fee account is not in the tree");
        let mut fee_circuit_account_balances =
            Vec::with_capacity(1 << models::params::BALANCE_TREE_DEPTH);
        for i in 0u32..1u32 << (models::params::BALANCE_TREE_DEPTH as u32) {
            let balance_value = fee_circuit_account
                .subtree
                .get(i)
                .map(|bal| bal.value)
                .unwrap_or_else(Fr::zero);
            fee_circuit_account_balances.push(Some(balance_value));
        }
        self.fee_account_balances = Some(fee_circuit_account_balances);

        let (mut root_after_fee, mut fee_account_witness) =
            crate::witness::utils::apply_fee(&mut self.account_tree, self.fee_account_id, 0, 0);
        for CollectedFee { token, amount } in fees {
            let (root, acc_witness) = crate::witness::utils::apply_fee(
                &mut self.account_tree,
                self.fee_account_id,
                u32::from(*token),
                big_decimal_to_u128(amount),
            );
            root_after_fee = root;
            fee_account_witness = acc_witness;
        }

        self.root_after_fees = Some(root_after_fee);
        self.fee_account_witness = Some(fee_account_witness);
    }

    // After fees collected creates public data commitment
    pub fn calculate_pubdata_commitment(&mut self) {
        let (fee_account_audit_path, _) =
            crate::witness::utils::get_audits(&self.account_tree, self.fee_account_id, 0);
        self.fee_account_audit_path = Some(fee_account_audit_path);

        let public_data_commitment = crate::witness::utils::public_data_commitment::<Engine>(
            &self.pubdata,
            Some(self.initial_root_hash),
            Some(
                self.root_after_fees
                    .expect("root after fee should be present at this step"),
            ),
            Some(Fr::from_str(&self.fee_account_id.to_string()).expect("failed to parse")),
            Some(Fr::from_str(&self.block_number.to_string()).unwrap()),
        );
        self.pubdata_commitment = Some(public_data_commitment);
    }

    // Finaly, creates circuit instance for given operations.
    pub fn into_circuit_instance(self) -> FranklinCircuit<'static, Engine> {
        let operation_batch_size = self.operations.len();
        FranklinCircuit {
            params: &models::params::JUBJUB_PARAMS,
            operation_batch_size,
            old_root: Some(self.initial_root_hash),
            new_root: Some(self.root_after_fees.expect("root after fee not present")),
            operations: self.operations,
            pub_data_commitment: Some(
                self.pubdata_commitment
                    .expect("pubdata commitment not present"),
            ),
            block_number: Some(Fr::from_str(&self.block_number.to_string()).unwrap()),
            validator_account: self
                .fee_account_witness
                .expect("fee account witness not present"),
            validator_address: Some(Fr::from_str(&self.fee_account_id.to_string()).unwrap()),
            validator_balances: self
                .fee_account_balances
                .expect("fee account balances not present"),
            validator_audit_path: self
                .fee_account_audit_path
                .expect("fee account audit path not present"),
        }
    }
}

pub fn check_circuit(circuit: FranklinCircuit<Engine>) {
    let mut cs = TestConstraintSystem::<Engine>::new();
    circuit.synthesize(&mut cs).unwrap();

    println!("unconstrained: {}", cs.find_unconstrained());
    println!("number of constraints {}", cs.num_constraints());
    if let Some(err) = cs.which_is_unsatisfied() {
        panic!("ERROR satisfying in {}", err);
    }
}

pub fn test_genesis_plasma_state(
    accounts: Vec<(AccountId, Account)>,
) -> (PlasmaState, WitnessAccumulator) {
    if accounts.iter().any(|(id, _)| *id == 0) {
        panic!("AccountId 0 is existing fee account");
    }

    let fee_account_id = 0;
    let validator_account = vec![(
        fee_account_id,
        Account::default_with_address(&Address::default()),
    )]
    .into_iter()
    .chain(accounts.into_iter())
    .collect();
    let plasma_state = PlasmaState::new(validator_account, 1);

    let mut circuit_account_tree =
        CircuitAccountTree::new(models::params::account_tree_depth() as u32);
    for (id, account) in plasma_state.get_accounts() {
        circuit_account_tree.insert(id, CircuitAccount::from(account))
    }

    let witness_accum = WitnessAccumulator::new(circuit_account_tree, fee_account_id, 1);

    (plasma_state, witness_accum)
}
