use zksync_crypto::{
    ff::{PrimeField, PrimeFieldRepr},
    Fr,
};

pub fn fr_to_bytes(scalar: Fr) -> Vec<u8> {
    let mut be_bytes = [0u8; 32];
    scalar
        .into_repr()
        .write_be(be_bytes.as_mut())
        .expect("Write commit bytes");

    be_bytes.to_vec()
}

pub fn fr_to_hex(scalar: Fr) -> String {
    let be_bytes = fr_to_bytes(scalar);

    hex::encode(be_bytes)
}
