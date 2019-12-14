pub mod client;
pub mod server;

// Built-in
use std::fmt;
use std::str::FromStr;
// External
use pairing::bn256::{Bn256, Fr};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
// Workspace
use circuit::account::AccountWitness;
use circuit::operation::{
    Operation, OperationArguments, OperationBranch, OperationBranchWitness, SignatureData,
};

#[derive(Serialize, Deserialize)]
pub struct ProverData2 {
    pub public_data_commitment: Fr,
    pub old_root: Fr,
    pub new_root: Fr,
    pub validator_address: Fr,
    #[serde(
        serialize_with = "vec_operations_ser",
        deserialize_with = "vec_operations_de"
    )]
    pub operations: Vec<circuit::operation::Operation<Bn256>>,
    #[serde(with = "AccountWitnessDef")]
    pub validator_account: circuit::account::AccountWitness<Bn256>,
}

pub fn vec_operations_ser<S: Serializer>(
    operations: &Vec<Operation<Bn256>>,
    ser: S,
) -> Result<S::Ok, S::Error> {
    #[derive(Serialize)]
    struct Wrapper(#[serde(with = "OperationDef")] Operation<Bn256>);

    let v = operations.into_iter().map(|a| Wrapper(a.clone())).collect();
    Vec::serialize(&v, ser)
}

fn vec_operations_de<'de, D>(deserializer: D) -> Result<Vec<Operation<Bn256>>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    struct Wrapper(#[serde(with = "OperationDef")] Operation<Bn256>);

    let v = Vec::deserialize(deserializer)?;
    Ok(v.into_iter().map(|Wrapper(a)| a.clone()).collect())
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "circuit::account::AccountWitness::<Bn256>")]
struct AccountWitnessDef {
    pub nonce: Option<Fr>,
    pub pub_key_hash: Option<Fr>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "circuit::operation::Operation::<Bn256>")]
pub struct OperationDef {
    pub new_root: Option<Fr>,
    pub tx_type: Option<Fr>,
    pub chunk: Option<Fr>,
    pub pubdata_chunk: Option<Fr>,
    pub signer_pub_key_packed: Vec<Option<bool>>,
    pub first_sig_msg: Option<Fr>,
    pub second_sig_msg: Option<Fr>,
    pub third_sig_msg: Option<Fr>,
    pub signature_data: SignatureData,
    #[serde(with = "OperationArgumentsDef")]
    pub args: OperationArguments<Bn256>,
    #[serde(with = "OperationBranchDef")]
    pub lhs: OperationBranch<Bn256>,
    #[serde(with = "OperationBranchDef")]
    pub rhs: OperationBranch<Bn256>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "circuit::operation::OperationArguments::<Bn256>")]
pub struct OperationArgumentsDef {
    pub a: Option<Fr>,
    pub b: Option<Fr>,
    pub amount_packed: Option<Fr>,
    pub full_amount: Option<Fr>,
    pub fee: Option<Fr>,
    pub new_pub_key_hash: Option<Fr>,
    pub ethereum_key: Option<Fr>,
    pub pub_nonce: Option<Fr>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "circuit::operation::OperationBranch::<Bn256>")]
pub struct OperationBranchDef {
    pub address: Option<Fr>,
    pub token: Option<Fr>,
    #[serde(with = "OperationBranchWitnessDef")]
    pub witness: OperationBranchWitness<Bn256>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "circuit::operation::OperationBranchWitness::<Bn256>")]
pub struct OperationBranchWitnessDef {
    #[serde(with = "AccountWitnessDef")]
    pub account_witness: AccountWitness<Bn256>,
    pub account_path: Vec<Option<Fr>>,

    pub balance_value: Option<Fr>,
    pub balance_subtree_path: Vec<Option<Fr>>,
}

/// ProverData is data prover needs to calculate proof of the given block.
#[derive(Clone, Debug)]
pub struct ProverData {
    pub public_data_commitment: Fr,
    pub old_root: Fr,
    pub new_root: Fr,
    pub validator_address: Fr,
    pub operations: Vec<circuit::operation::Operation<Bn256>>,
    pub validator_balances: Vec<Option<Fr>>,
    pub validator_audit_path: Vec<Option<Fr>>,
    pub validator_account: circuit::account::AccountWitness<Bn256>,
}
