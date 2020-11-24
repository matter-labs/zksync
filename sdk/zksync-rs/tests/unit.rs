use std::collections::HashMap;
use zksync::{tokens_cache::TokensCache, utils::*, web3::types::H160, zksync_types::Token};
use zksync_config::test_config::unit_vectors::{Config as TestVectorsConfig, TestEntry};
use zksync_crypto::PrivateKey;
use zksync_types::tx::TxSignature;

#[test]
fn test_tokens_cache() {
    let mut tokens: HashMap<String, Token> = HashMap::default();

    let token_eth = Token::new(0, H160::default(), "ETH", 18);
    tokens.insert("ETH".to_string(), token_eth.clone());
    let token_dai = Token::new(1, H160::random(), "DAI", 18);
    tokens.insert("DAI".to_string(), token_dai.clone());

    let uncahed_token = Token::new(2, H160::random(), "UNC", 5);

    let tokens_hash = TokensCache::new(tokens);

    assert_eq!(
        tokens_hash.resolve(token_eth.address.into()),
        Some(token_eth.clone())
    );
    assert_eq!(
        tokens_hash.resolve(token_eth.id.into()),
        Some(token_eth.clone())
    );
    assert_eq!(
        tokens_hash.resolve((&token_eth.symbol as &str).into()),
        Some(token_eth.clone())
    );

    assert_eq!(
        tokens_hash.resolve(token_dai.address.into()),
        Some(token_dai.clone())
    );
    assert_eq!(
        tokens_hash.resolve(token_dai.id.into()),
        Some(token_dai.clone())
    );
    assert_eq!(
        tokens_hash.resolve((&token_dai.symbol as &str).into()),
        Some(token_dai.clone())
    );

    assert_eq!(tokens_hash.resolve(uncahed_token.address.into()), None);
    assert_eq!(tokens_hash.resolve(uncahed_token.id.into()), None);
    assert_eq!(
        tokens_hash.resolve((&uncahed_token.symbol as &str).into()),
        None
    );

    assert!(tokens_hash.is_eth(token_eth.address.into()));
    assert!(tokens_hash.is_eth(token_eth.id.into()));
    assert!(tokens_hash.is_eth((&token_eth.symbol as &str).into()));

    assert!(!tokens_hash.is_eth(token_dai.address.into()));
    assert!(!tokens_hash.is_eth(token_dai.id.into()));
    assert!(!tokens_hash.is_eth((&token_dai.symbol as &str).into()));
}

fn priv_key_from_raw(raw: &[u8]) -> Option<PrivateKey> {
    use zksync_crypto::{
        bellman::{pairing::ff::PrimeField, PrimeFieldRepr},
        franklin_crypto::alt_babyjubjub::fs::FsRepr,
        priv_key_from_fs, Fs,
    };

    let mut fs_repr = FsRepr::default();
    fs_repr.read_be(raw).ok()?;
    Fs::from_repr(fs_repr).ok().map(priv_key_from_fs)
}

fn assert_tx_signature(signature: &TxSignature, expected_pub: &str, expected_sig: &str) {
    let TxSignature { pub_key, signature } = signature;

    let pub_point = pub_key.serialize_packed().unwrap();
    assert_eq!(hex::encode(pub_point), expected_pub);

    let packed_sig = signature.serialize_packed().unwrap();
    assert_eq!(hex::encode(packed_sig), expected_sig);
}

#[cfg(test)]
mod primitives_with_vectors {
    use super::*;

    use zksync_config::test_config::unit_vectors::Config as TestVectorsConfig;

    #[test]
    fn test_signature() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.crypto_primitives.items {
            let private_key =
                private_key_from_seed(&inputs.seed).expect("Cannot get key from seed");

            assert_eq!(
                priv_key_from_raw(&outputs.private_key).unwrap().0,
                private_key.0
            );

            let signature = TxSignature::sign_musig(&private_key, &inputs.message);
            assert_tx_signature(&signature, &outputs.pub_key_hash, &outputs.signature);
        }
    }
}

#[cfg(test)]
mod utils_with_vectors {
    use super::*;
    use zksync_utils::format_units;

    #[test]
    fn test_token_packing() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.utils.amount_packing.items {
            let token_amount = inputs.value;

            assert_eq!(is_token_amount_packable(&token_amount), outputs.packable);
            assert_eq!(
                closest_packable_token_amount(&token_amount),
                outputs.closest_packable
            );
            assert_eq!(pack_token_amount(&token_amount), outputs.packed_value);
        }
    }

    #[test]
    fn test_fee_packing() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.utils.fee_packing.items {
            let fee_amount = inputs.value;

            assert_eq!(is_fee_amount_packable(&fee_amount), outputs.packable);
            assert_eq!(
                closest_packable_fee_amount(&fee_amount),
                outputs.closest_packable
            );
            assert_eq!(pack_fee_amount(&fee_amount), outputs.packed_value);
        }
    }

    #[test]
    fn test_formatting() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.utils.token_formatting.items {
            let units_str = format_units(inputs.amount, inputs.decimals);
            assert_eq!(format!("{} {}", units_str, inputs.token), outputs.formatted);
        }
    }
}
