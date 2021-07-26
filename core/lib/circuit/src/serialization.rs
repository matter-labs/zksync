// Built-in
// External
use serde::{Deserialize, Deserializer, Serialize, Serializer};
// Workspace
use zksync_crypto::ff::PrimeField;
use zksync_crypto::franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use zksync_crypto::franklin_crypto::rescue::bn256::Bn256RescueParams;
use zksync_crypto::serialization::*;
use zksync_crypto::{Engine, Fr};
// Local
use crate::account::AccountWitness;
use crate::circuit::ZkSyncCircuit;
use crate::operation::{
    Operation, OperationArguments, OperationBranch, OperationBranchWitness, SignatureData,
};
use crate::witness::WitnessBuilder;

/// ProverData is data prover needs to calculate proof of the given block.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProverData {
    #[serde(with = "FrSerde")]
    pub block_number: Fr,
    #[serde(with = "FrSerde")]
    pub public_data_commitment: Fr,
    #[serde(with = "FrSerde")]
    pub old_root: Fr,
    #[serde(with = "FrSerde")]
    pub initial_used_subtree_root: Fr,
    #[serde(with = "FrSerde")]
    pub new_root: Fr,
    #[serde(with = "FrSerde")]
    pub block_timestamp: Fr,
    #[serde(with = "FrSerde")]
    pub validator_address: Fr,
    #[serde(with = "VecOptionalFrSerde")]
    pub validator_balances: Vec<Option<Fr>>,
    #[serde(with = "VecOptionalFrSerde")]
    pub validator_audit_path: Vec<Option<Fr>>,
    #[serde(with = "VecOperationsSerde")]
    pub operations: Vec<Operation<Engine>>,
    #[serde(with = "AccountWitnessDef")]
    pub validator_account: AccountWitness<Engine>,
    #[serde(with = "VecOptionalFrSerde")]
    pub validator_non_processable_tokens_audit_before_fees: Vec<Option<Fr>>,
    #[serde(with = "VecOptionalFrSerde")]
    pub validator_non_processable_tokens_audit_after_fees: Vec<Option<Fr>>,
}

impl From<WitnessBuilder<'_>> for ProverData {
    fn from(witness_builder: WitnessBuilder) -> ProverData {
        ProverData {
            public_data_commitment: witness_builder.pubdata_commitment.unwrap(),
            old_root: witness_builder.initial_root_hash,
            initial_used_subtree_root: witness_builder.initial_used_subtree_root_hash,
            new_root: witness_builder.root_after_fees.unwrap(),
            block_timestamp: Fr::from_str(&witness_builder.timestamp.to_string())
                .expect("failed to parse"),
            block_number: Fr::from_str(&witness_builder.block_number.to_string())
                .expect("failed to parse"),
            validator_address: Fr::from_str(&witness_builder.fee_account_id.to_string())
                .expect("failed to parse"),
            operations: witness_builder.operations,
            validator_balances: witness_builder.fee_account_balances.unwrap(),
            validator_audit_path: witness_builder.fee_account_audit_path.unwrap(),
            validator_account: witness_builder.fee_account_witness.unwrap(),
            validator_non_processable_tokens_audit_before_fees: witness_builder
                .validator_non_processable_tokens_audit_before_fees
                .unwrap(),
            validator_non_processable_tokens_audit_after_fees: witness_builder
                .validator_non_processable_tokens_audit_after_fees
                .unwrap(),
        }
    }
}

impl ProverData {
    pub fn into_circuit(self) -> ZkSyncCircuit<'static, Engine> {
        ZkSyncCircuit {
            rescue_params: &zksync_crypto::params::RESCUE_PARAMS as &Bn256RescueParams,
            jubjub_params: &zksync_crypto::params::JUBJUB_PARAMS as &AltJubjubBn256,
            old_root: Some(self.old_root),
            initial_used_subtree_root: Some(self.initial_used_subtree_root),
            block_number: Some(self.block_number),
            block_timestamp: Some(self.block_timestamp),
            validator_address: Some(self.validator_address),
            pub_data_commitment: Some(self.public_data_commitment),
            operations: self.operations,
            validator_balances: self.validator_balances,
            validator_audit_path: self.validator_audit_path,
            validator_account: self.validator_account,
            validator_non_processable_tokens_audit_before_fees: self
                .validator_non_processable_tokens_audit_before_fees,
            validator_non_processable_tokens_audit_after_fees: self
                .validator_non_processable_tokens_audit_after_fees,
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::account::AccountWitness::<Engine>")]
struct AccountWitnessDef {
    #[serde(with = "OptionalFrSerde")]
    pub nonce: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub pub_key_hash: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub address: Option<Fr>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::operation::Operation::<Engine>")]
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
#[serde(remote = "crate::operation::OperationArguments::<Engine>")]
pub struct OperationArgumentsDef {
    #[serde(with = "OptionalFrSerde")]
    pub a: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub b: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub amount_packed: Option<Fr>,
    #[serde(with = "VecOptionalFrSerde")]
    pub special_content_hash: Vec<Option<Fr>>,
    #[serde(with = "OptionalFrSerde")]
    pub special_serial_id: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub second_amount_packed: Option<Fr>,
    #[serde(with = "VecOptionalFrSerde")]
    pub special_tokens: Vec<Option<Fr>>,
    #[serde(with = "VecOptionalFrSerde")]
    pub special_accounts: Vec<Option<Fr>>,
    #[serde(with = "VecOptionalFrSerde")]
    pub special_amounts: Vec<Option<Fr>>,
    #[serde(with = "VecOptionalFrSerde")]
    pub special_nonces: Vec<Option<Fr>>,
    #[serde(with = "VecOptionalFrSerde")]
    pub special_prices: Vec<Option<Fr>>,
    #[serde(with = "VecOptionalFrSerde")]
    pub special_eth_addresses: Vec<Option<Fr>>,
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
    #[serde(with = "OptionalFrSerde")]
    pub valid_from: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub valid_until: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub second_valid_from: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub second_valid_until: Option<Fr>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::operation::OperationBranch::<Engine>")]
pub struct OperationBranchDef {
    #[serde(with = "OptionalFrSerde")]
    pub address: Option<Fr>,
    #[serde(with = "OptionalFrSerde")]
    pub token: Option<Fr>,
    #[serde(with = "OperationBranchWitnessDef")]
    pub witness: OperationBranchWitness<Engine>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "crate::operation::OperationBranchWitness::<Engine>")]
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

pub struct VecOperationsSerde;

impl VecOperationsSerde {
    pub fn serialize<S>(operations: &[Operation<Engine>], ser: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        #[derive(Serialize)]
        struct Wrapper(#[serde(with = "OperationDef")] Operation<Engine>);

        let v = operations.iter().map(|a| Wrapper(a.clone())).collect();
        Vec::serialize(&v, ser)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<Operation<Engine>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper(#[serde(with = "OperationDef")] Operation<Engine>);

        let v = Vec::deserialize(deserializer)?;
        Ok(v.into_iter().map(|Wrapper(a)| a).collect())
    }
}
