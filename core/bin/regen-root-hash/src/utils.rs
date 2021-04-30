use zksync_crypto::{
    ff::{PrimeField, PrimeFieldRepr},
    Fr,
};
use zksync_types::{tx::PackedEthSignature, H256};

pub fn fr_to_hex(scalar: Fr) -> String {
    let mut be_bytes = [0u8; 32];
    scalar
        .into_repr()
        .write_be(be_bytes.as_mut())
        .expect("Write commit bytes");
    hex::encode(be_bytes)
}

pub fn get_message_to_sign(old_hash: Fr, new_hash: Fr) -> String {
    let old_hash_str = fr_to_hex(old_hash).to_ascii_lowercase();
    let new_hash_str = fr_to_hex(new_hash).to_ascii_lowercase();

    let message = format!(
        "OldRootHash:0x{},NewRootHash:0x{}",
        old_hash_str, new_hash_str
    );

    message
}

pub fn sign_message(private_key_str: String, message: String) -> String {
    let pk_bytes = hex::decode(private_key_str).unwrap();
    let pk = H256::from_slice(&pk_bytes);

    let message_bytes = message.as_bytes();

    let signature = PackedEthSignature::sign(&pk, message_bytes).unwrap();

    hex::encode(signature.serialize_packed())
}
