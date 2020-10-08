// Built-in
// External
use serde::{Deserialize, Serialize};
// Workspace
use zksync_circuit::account::AccountWitness;
use zksync_circuit::circuit::ZkSyncCircuit;
use zksync_circuit::operation::{
    OperationArguments, OperationBranch, OperationBranchWitness, SignatureData,
};
use zksync_crypto::ff::PrimeField;
use zksync_crypto::franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use zksync_crypto::franklin_crypto::rescue::bn256::Bn256RescueParams;
use zksync_crypto::{Engine, Fr};
// Local
use crate::serialization::*;
use zksync_circuit::witness::WitnessBuilder;

/// ProverData is data prover needs to calculate proof of the given block.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProverData {
    #[serde(with = "FrSerde")]
    pub public_data_commitment: Fr,
    #[serde(with = "FrSerde")]
    pub old_root: Fr,
    #[serde(with = "FrSerde")]
    pub initial_used_subtree_root: Fr,
    #[serde(with = "FrSerde")]
    pub new_root: Fr,
    #[serde(with = "FrSerde")]
    pub validator_address: Fr,
    #[serde(with = "VecOptionalFrSerde")]
    pub validator_balances: Vec<Option<Fr>>,
    #[serde(with = "VecOptionalFrSerde")]
    pub validator_audit_path: Vec<Option<Fr>>,
    #[serde(with = "VecOperationsSerde")]
    pub operations: Vec<zksync_circuit::operation::Operation<Engine>>,
    #[serde(with = "AccountWitnessDef")]
    pub validator_account: zksync_circuit::account::AccountWitness<Engine>,
}

impl From<WitnessBuilder<'_>> for ProverData {
    fn from(witness_builder: WitnessBuilder) -> ProverData {
        ProverData {
            public_data_commitment: witness_builder.pubdata_commitment.unwrap(),
            old_root: witness_builder.initial_root_hash,
            initial_used_subtree_root: witness_builder.initial_used_subtree_root_hash,
            new_root: witness_builder.root_after_fees.unwrap(),
            validator_address: Fr::from_str(&witness_builder.fee_account_id.to_string())
                .expect("failed to parse"),
            operations: witness_builder.operations,
            validator_balances: witness_builder.fee_account_balances.unwrap(),
            validator_audit_path: witness_builder.fee_account_audit_path.unwrap(),
            validator_account: witness_builder.fee_account_witness.unwrap(),
        }
    }
}

impl ProverData {
    pub fn into_circuit(self, block: i64) -> ZkSyncCircuit<'static, Engine> {
        ZkSyncCircuit {
            rescue_params: &zksync_crypto::params::RESCUE_PARAMS as &Bn256RescueParams,
            jubjub_params: &zksync_crypto::params::JUBJUB_PARAMS as &AltJubjubBn256,
            old_root: Some(self.old_root),
            initial_used_subtree_root: Some(self.initial_used_subtree_root),
            block_number: Fr::from_str(&block.to_string()),
            validator_address: Some(self.validator_address),
            pub_data_commitment: Some(self.public_data_commitment),
            operations: self.operations,
            validator_balances: self.validator_balances,
            validator_audit_path: self.validator_audit_path,
            validator_account: self.validator_account,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "zksync_circuit::account::AccountWitness::<Engine>")]
struct AccountWitnessDef {
    #[serde(with = "OptionalFrSerde")]
    pub nonce: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub pub_key_hash: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub address: Option<Fr>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "zksync_circuit::operation::Operation::<Engine>")]
pub struct OperationDef {
    #[serde(with = "OptionalFrSerde")]
    pub new_root: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub tx_type: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub chunk: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub pubdata_chunk: Option<Fr>,

    pub signer_pub_key_packed: Vec<Option<bool>>,
    #[serde(with = "OptionalFrSerde")]
    pub first_sig_msg: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub second_sig_msg: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub third_sig_msg: Option<Fr>,
    pub signature_data: SignatureData,
    #[serde(with = "OperationArgumentsDef")]
    pub args: OperationArguments<Engine>,
    #[serde(with = "OperationBranchDef")]
    pub lhs: OperationBranch<Engine>,
    #[serde(with = "OperationBranchDef")]
    pub rhs: OperationBranch<Engine>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "zksync_circuit::operation::OperationArguments::<Engine>")]
pub struct OperationArgumentsDef {
    #[serde(with = "OptionalFrSerde")]
    pub a: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub b: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub amount_packed: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub full_amount: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub fee: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub new_pub_key_hash: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub eth_address: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub pub_nonce: Option<Fr>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "zksync_circuit::operation::OperationBranch::<Engine>")]
pub struct OperationBranchDef {
    #[serde(with = "OptionalFrSerde")]
    pub address: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub token: Option<Fr>,
    #[serde(with = "OperationBranchWitnessDef")]
    pub witness: OperationBranchWitness<Engine>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "zksync_circuit::operation::OperationBranchWitness::<Engine>")]
pub struct OperationBranchWitnessDef {
    #[serde(with = "AccountWitnessDef")]
    pub account_witness: AccountWitness<Engine>,
    #[serde(with = "VecOptionalFrSerde")]
    pub account_path: Vec<Option<Fr>>,
    #[serde(with = "OptionalFrSerde")]
    pub balance_value: Option<Fr>,
    #[serde(with = "VecOptionalFrSerde")]
    pub balance_subtree_path: Vec<Option<Fr>>,
}
