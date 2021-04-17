use parity_crypto::{publickey::sign, Keccak256};
use structopt::StructOpt;
use zksync_crypto::{
    ff::{PrimeField, PrimeFieldRepr},
    Fr,
};
use zksync_types::H256;

#[derive(StructOpt)]
pub struct Params {
    /// The current root hash (balance subtree depth 11)
    #[structopt(short = "h")]
    pub current_root_hash: String,

    /// The path to the JSON dump of the accounts table
    #[structopt(short = "a")]
    pub accounts_dump: String,

    /// The path to the JSON dump of the accounts table
    #[structopt(short = "b")]
    pub balances_dump: String,

    /// The private key of the signer
    #[structopt(short = "pk")]
    pub private_key: String,
}

pub fn fr_to_hex(scalar: Fr) -> String {
    let mut be_bytes = [0u8; 32];
    scalar
        .into_repr()
        .write_be(be_bytes.as_mut())
        .expect("Write commit bytes");
    hex::encode(be_bytes)
}

pub fn sign_update_message(private_key_str: String, old_hash: Fr, new_hash: Fr) -> String {
    let pk_bytes = hex::decode(private_key_str).unwrap();
    let pk = H256::from_slice(&pk_bytes);

    let old_hash_str = fr_to_hex(old_hash);
    let new_hash_str = fr_to_hex(new_hash);

    let message = format!("OldRootHash:{},NewRootHash:{}", old_hash_str, new_hash_str);

    let message_hash: H256 = message.as_bytes().keccak256().into();

    let signature = sign(&pk.into(), &message_hash).unwrap();

    signature.to_string()
}
