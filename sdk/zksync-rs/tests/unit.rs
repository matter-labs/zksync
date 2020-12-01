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

#[cfg(test)]
mod signatures_with_vectors {
    use super::*;
    use zksync::{signer::Signer, WalletCredentials};
    use zksync_config::test_config::unit_vectors::{EthSignature, Tx};
    use zksync_eth_signer::PrivateKeySigner;
    use zksync_types::{network::Network, AccountId, Address, H256};

    async fn get_signer(
        eth_private_key_raw: &[u8],
        from_address: Address,
        account_id: AccountId,
    ) -> Signer<PrivateKeySigner> {
        let eth_private_key = H256::from_slice(eth_private_key_raw);
        let eth_signer = PrivateKeySigner::new(eth_private_key);

        let creds = WalletCredentials::from_eth_signer(from_address, eth_signer, Network::Mainnet)
            .await
            .unwrap();

        let mut signer = Signer::with_credentials(creds);
        signer.set_account_id(Some(account_id));
        signer
    }

    #[tokio::test]
    async fn test_transfer_signature() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.transactions.items {
            if let Tx::Transfer(transfer_tx) = &inputs.tx {
                let sign_data = if let EthSignature::Transfer(sign_data) = inputs.eth_sign_data {
                    sign_data
                } else {
                    panic!("Signature data does not match transaction type (transfer)")
                };

                let signer = get_signer(
                    &inputs.eth_private_key,
                    transfer_tx.from,
                    sign_data.account_id,
                )
                .await;

                let token = Token {
                    id: transfer_tx.token_id,
                    address: Default::default(),
                    symbol: sign_data.string_token.clone(),
                    decimals: 0,
                };
                let (transfer, eth_signature) = signer
                    .sign_transfer(
                        token,
                        transfer_tx.amount.clone(),
                        transfer_tx.fee.clone(),
                        sign_data.to,
                        sign_data.nonce,
                    )
                    .await
                    .expect("Transfer signing error");

                assert_eq!(transfer.get_bytes(), outputs.sign_bytes);
                assert_tx_signature(
                    &transfer.signature,
                    &outputs.signature.pub_key,
                    &outputs.signature.signature,
                );

                assert_eq!(
                    transfer.get_ethereum_sign_message(&sign_data.string_token, 0),
                    outputs.eth_sign_message.unwrap()
                );

                if let Some(expected_eth_signature) = outputs.eth_signature {
                    let eth_signature = eth_signature.unwrap().serialize_packed();
                    assert_eq!(&eth_signature, expected_eth_signature.as_slice());
                }
            }
        }
    }

    #[tokio::test]
    async fn test_withdraw_signature() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.transactions.items {
            if let Tx::Withdraw(withdraw_tx) = &inputs.tx {
                let sign_data = if let EthSignature::Withdraw(sign_data) = inputs.eth_sign_data {
                    sign_data
                } else {
                    panic!("Signature data does not match transaction type (withdraw)")
                };

                let signer = get_signer(
                    &inputs.eth_private_key,
                    withdraw_tx.from,
                    sign_data.account_id,
                )
                .await;

                let token = Token {
                    id: withdraw_tx.token_id,
                    address: Default::default(),
                    symbol: sign_data.string_token.clone(),
                    decimals: 0,
                };
                let (withdraw, eth_signature) = signer
                    .sign_withdraw(
                        token,
                        withdraw_tx.amount.clone(),
                        withdraw_tx.fee.clone(),
                        sign_data.eth_address,
                        sign_data.nonce,
                    )
                    .await
                    .expect("Withdraw signing error");

                assert_eq!(withdraw.get_bytes(), outputs.sign_bytes);
                assert_tx_signature(
                    &withdraw.signature,
                    &outputs.signature.pub_key,
                    &outputs.signature.signature,
                );

                assert_eq!(
                    withdraw.get_ethereum_sign_message(&sign_data.string_token, 0),
                    outputs.eth_sign_message.unwrap()
                );

                if let Some(expected_eth_signature) = outputs.eth_signature {
                    let eth_signature = eth_signature.unwrap().serialize_packed();
                    assert_eq!(&eth_signature, expected_eth_signature.as_slice());
                }
            }
        }
    }

    #[tokio::test]
    async fn test_change_pubkey_signature() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.transactions.items {
            if let Tx::ChangePubKey(change_pubkey_tx) = &inputs.tx {
                let sign_data = if let EthSignature::ChangePubKey(sign_data) = inputs.eth_sign_data
                {
                    sign_data
                } else {
                    panic!("Signature data does not match transaction type (change pub key)")
                };

                let mut signer = get_signer(
                    &inputs.eth_private_key,
                    change_pubkey_tx.account,
                    sign_data.account_id,
                )
                .await;
                signer.pubkey_hash = change_pubkey_tx.new_pk_hash.clone();

                let token = Token {
                    id: change_pubkey_tx.fee_token_id,
                    address: Default::default(),
                    symbol: String::new(),
                    decimals: 0,
                };
                let change_pub_key = signer
                    .sign_change_pubkey_tx(
                        sign_data.nonce,
                        false,
                        token,
                        change_pubkey_tx.fee.clone(),
                    )
                    .await
                    .expect("Change pub key signing error");

                assert_eq!(change_pub_key.get_bytes(), outputs.sign_bytes);
                assert_tx_signature(
                    &change_pub_key.signature,
                    &outputs.signature.pub_key,
                    &outputs.signature.signature,
                );

                assert_eq!(
                    change_pub_key.get_eth_signed_data().unwrap(),
                    outputs.eth_sign_message.unwrap().into_bytes()
                );

                if let Some(expected_eth_signature) = outputs.eth_signature {
                    let eth_signature = change_pub_key.eth_signature.unwrap().serialize_packed();
                    assert_eq!(&eth_signature, expected_eth_signature.as_slice());
                }
            }
        }
    }

    #[tokio::test]
    async fn test_forced_exit_signature() {
        let test_vectors = TestVectorsConfig::load();
        for TestEntry { inputs, outputs } in test_vectors.transactions.items {
            if let Tx::ForcedExit(forced_exit) = &inputs.tx {
                if let EthSignature::ForcedExit = inputs.eth_sign_data {
                } else {
                    panic!("Signature data does not match transaction type (forced exit)")
                }

                let signer = get_signer(
                    &inputs.eth_private_key,
                    forced_exit.from,
                    forced_exit.initiator_account_id,
                )
                .await;

                let token = Token {
                    id: forced_exit.token_id,
                    address: Default::default(),
                    symbol: String::new(),
                    decimals: 0,
                };
                let forced_exit = signer
                    .sign_forced_exit(
                        forced_exit.target,
                        token,
                        forced_exit.fee.clone(),
                        forced_exit.nonce,
                    )
                    .await
                    .expect("Forced exit signing error");

                assert_eq!(forced_exit.get_bytes(), outputs.sign_bytes);
                assert_tx_signature(
                    &forced_exit.signature,
                    &outputs.signature.pub_key,
                    &outputs.signature.signature,
                );
            }
        }
    }
}
