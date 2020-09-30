//! Compare crypto primitives to those that we use in our `zksync_types` crate;

use super::{private_key_to_pubkey_hash, read_signing_key, sign_musig};

use crypto_lib::{public_key_from_private, Engine};
use franklin_crypto::bellman::pairing::ff::{self, PrimeField, PrimeFieldRepr};
use franklin_crypto::eddsa::PrivateKey;
use rand::{Rng, SeedableRng, XorShiftRng};
use zksync_types::{tx::TxSignature, PubKeyHash};

fn gen_private_key_and_its_be_bytes() -> (PrivateKey<Engine>, Vec<u8>) {
    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);

    let pk = PrivateKey::<Engine>(rng.gen());
    let mut serialized_key = Vec::new();
    pk.0.into_repr()
        .write_be(&mut serialized_key)
        .expect("private key write");
    (pk, serialized_key)
}

#[test]
fn test_private_key_read() {
    let (zksync_types_pk, serialized_pk) = gen_private_key_and_its_be_bytes();

    let wasm_pk = read_signing_key(&serialized_pk);
    assert_eq!(ff::to_hex(&wasm_pk.0), ff::to_hex(&zksync_types_pk.0));
}

#[test]
fn test_pubkey_hash() {
    let (pk, serialized_pk) = gen_private_key_and_its_be_bytes();

    let wasm_pubkey_hash = private_key_to_pubkey_hash(&serialized_pk);
    let zksync_types_pubkey_hash = PubKeyHash::from_privkey(&pk).data.to_vec();
    assert_eq!(wasm_pubkey_hash, zksync_types_pubkey_hash);
}

#[test]
fn test_signature() {
    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);
    let mut random_msg = |len| rng.gen_iter::<u8>().take(len).collect::<Vec<_>>();

    let (pk, serialized_pk) = gen_private_key_and_its_be_bytes();
    let pubkey = public_key_from_private(&pk);

    for msg_len in &[0, 2, 4, 5, 32, 128] {
        let msg = random_msg(*msg_len);

        let wasm_signature = sign_musig(&serialized_pk, &msg);

        let wasm_unpacked_signature = TxSignature::deserialize_from_packed_bytes(&wasm_signature)
            .expect("failed to unpack signature");

        let signer_pubkey = wasm_unpacked_signature.verify_musig(&msg);
        assert_eq!(
            signer_pubkey.map(|pk| pk.0.into_xy()),
            Some(pubkey.0.into_xy()),
            "msg_len: {}, msg_hex: {}, wasm_signature_hex:{}",
            msg_len,
            hex::encode(&msg),
            hex::encode(&wasm_signature)
        );
    }
}
