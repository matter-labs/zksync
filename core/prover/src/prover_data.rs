// Built-in
// External
use serde::{Deserialize, Deserializer, Serialize, Serializer};
// Workspace
use circuit::account::AccountWitness;
use circuit::operation::{
    ETHSignatureData, Operation, OperationArguments, OperationBranch, OperationBranchWitness,
    SignatureData,
};
use models::node::{Engine, Fr};

/// ProverData is data prover needs to calculate proof of the given block.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProverData {
    pub public_data_commitment: Fr,
    pub old_root: Fr,
    pub new_root: Fr,
    pub validator_address: Fr,
    pub validator_balances: Vec<Option<Fr>>,
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
    pub nonce: Option<Fr>,
    pub pub_key_hash: Option<Fr>,
    pub address: Option<Fr>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "circuit::operation::ETHSignatureData::<Engine>")]
pub struct ETHSignatureDataDef {
    pub r: Vec<Option<bool>>,
    pub s: Vec<Option<bool>>,
    pub v: Option<Fr>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "circuit::operation::Operation::<Engine>")]
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
    #[serde(with = "ETHSignatureDataDef")]
    pub eth_signature_data: ETHSignatureData<Engine>,
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
#[serde(remote = "circuit::operation::OperationBranch::<Engine>")]
pub struct OperationBranchDef {
    pub address: Option<Fr>,
    pub token: Option<Fr>,
    #[serde(with = "OperationBranchWitnessDef")]
    pub witness: OperationBranchWitness<Engine>,
}

#[derive(Serialize, Deserialize)]
#[serde(remote = "circuit::operation::OperationBranchWitness::<Engine>")]
pub struct OperationBranchWitnessDef {
    #[serde(with = "AccountWitnessDef")]
    pub account_witness: AccountWitness<Engine>,
    pub account_path: Vec<Option<Fr>>,

    pub balance_value: Option<Fr>,
    pub balance_subtree_path: Vec<Option<Fr>>,
}
