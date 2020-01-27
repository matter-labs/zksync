// Built-in
// External
use serde::{Deserialize, Deserializer, Serialize, Serializer};
// Workspace
use circuit::account::AccountWitness;
use circuit::operation::{
    Operation, OperationArguments, OperationBranch, OperationBranchWitness, SignatureData,
};
use models::node::{Engine, Fr};
use models::serialization::*;

/// ProverData is data prover needs to calculate proof of the given block.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProverData {
    #[serde(
        serialize_with = "fr_ser",
        deserialize_with = "fr_de"
    )]
    pub public_data_commitment: Fr,
    #[serde(
        serialize_with = "fr_ser",
        deserialize_with = "fr_de"
    )]
    pub old_root: Fr,
    #[serde(
        serialize_with = "fr_ser",
        deserialize_with = "fr_de"
    )]
    pub new_root: Fr,
    #[serde(
        serialize_with = "fr_ser",
        deserialize_with = "fr_de"
    )]
    pub validator_address: Fr,
    #[serde(
        serialize_with = "vec_optional_fr_ser",
        deserialize_with = "vec_optional_fr_de"
    )]
    pub validator_balances: Vec<Option<Fr>>,
    #[serde(
        serialize_with = "vec_optional_fr_ser",
        deserialize_with = "vec_optional_fr_de"
    )]
    pub validator_audit_path: Vec<Option<Fr>>,
    #[serde(
        serialize_with = "vec_operations_ser",
        deserialize_with = "vec_operations_de"
    )]
    pub operations: Vec<circuit::operation::Operation<Engine>>,
    #[serde(with = "AccountWitnessDef")]
    pub validator_account: circuit::account::AccountWitness<Engine>,
}

pub fn vec_operations_ser<S: Serializer>(
    operations: &[Operation<Engine>],
    ser: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Wrapper(#[serde(with = "OperationDef")] Operation<Engine>);

    let v = operations.iter().map(|a| Wrapper(a.clone())).collect();
    Vec::serialize(&v, ser)
}

fn vec_operations_de<'de, D>(deserializer: D) -> Result<Vec<Operation<Engine>>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Wrapper(#[serde(with = "OperationDef")] Operation<Engine>);

    let v = Vec::deserialize(deserializer)?;
    Ok(v.into_iter().map(|Wrapper(a)| a.clone()).collect())
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "circuit::account::AccountWitness::<Engine>")]
struct AccountWitnessDef {
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub nonce: Option<Fr>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub pub_key_hash: Option<Fr>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "circuit::operation::Operation::<Engine>")]
pub struct OperationDef {
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub new_root: Option<Fr>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub tx_type: Option<Fr>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub chunk: Option<Fr>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub pubdata_chunk: Option<Fr>,

    pub signer_pub_key_packed: Vec<Option<bool>>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub first_sig_msg: Option<Fr>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub second_sig_msg: Option<Fr>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
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
#[serde(remote = "circuit::operation::OperationArguments::<Engine>")]
pub struct OperationArgumentsDef {
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub a: Option<Fr>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub b: Option<Fr>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub amount_packed: Option<Fr>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub full_amount: Option<Fr>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub fee: Option<Fr>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub new_pub_key_hash: Option<Fr>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub ethereum_key: Option<Fr>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub pub_nonce: Option<Fr>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "circuit::operation::OperationBranch::<Engine>")]
pub struct OperationBranchDef {
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub address: Option<Fr>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub token: Option<Fr>,
    #[serde(with = "OperationBranchWitnessDef")]
    pub witness: OperationBranchWitness<Engine>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "circuit::operation::OperationBranchWitness::<Engine>")]
pub struct OperationBranchWitnessDef {
    #[serde(with = "AccountWitnessDef")]
    pub account_witness: AccountWitness<Engine>,
    #[serde(
        serialize_with = "vec_optional_fr_ser",
        deserialize_with = "vec_optional_fr_de"
    )]
    pub account_path: Vec<Option<Fr>>,
    #[serde(
        serialize_with = "optional_fr_ser",
        deserialize_with = "optional_fr_de"
    )]
    pub balance_value: Option<Fr>,
    #[serde(
        serialize_with = "vec_optional_fr_ser",
        deserialize_with = "vec_optional_fr_de"
    )]
    pub balance_subtree_path: Vec<Option<Fr>>,
}
