use num::{BigUint, ToPrimitive};
use serde::{Deserialize, Serialize};

use zksync_basic_types::Address;
use zksync_crypto::franklin_crypto::{
    eddsa::{PrivateKey, PublicKey},
    jubjub::FixedGenerators,
};
use zksync_crypto::params::{max_account_id, max_token_id, JUBJUB_PARAMS};
use zksync_crypto::public_key_from_private;
use zksync_crypto::rand::{Rng, SeedableRng, XorShiftRng};

use super::*;
use crate::{
    helpers::{pack_fee_amount, pack_token_amount},
    AccountId, Engine, TokenId,
};

fn gen_pk_and_msg() -> (PrivateKey<Engine>, Vec<Vec<u8>>) {
    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);

    let pk = PrivateKey(rng.gen());

    let mut messages = Vec::new();
    messages.push(Vec::<u8>::new());
    messages.push(b"hello world".to_vec());

    (pk, messages)
}

fn gen_account_id<T: Rng>(rng: &mut T) -> AccountId {
    rng.gen::<u32>().min(max_account_id())
}

fn gen_token_id<T: Rng>(rng: &mut T) -> TokenId {
    rng.gen::<u16>().min(max_token_id())
}

#[test]
fn test_print_transfer_for_protocol() {
    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);
    let key = gen_pk_and_msg().0;
    let transfer = Transfer::new_signed(
        gen_account_id(&mut rng),
        Address::from(rng.gen::<[u8; 20]>()),
        Address::from(rng.gen::<[u8; 20]>()),
        gen_token_id(&mut rng),
        BigUint::from(12_340_000_000_000u64),
        BigUint::from(56_700_000_000u64),
        rng.gen(),
        &key,
    )
    .expect("failed to sign transfer");

    println!(
        "User representation:\n{}\n",
        serde_json::to_string_pretty(&transfer).expect("json serialize")
    );

    println!("Signer:");
    println!("Private key: {}", key.0.to_string());
    let (pk_x, pk_y) = public_key_from_private(&key).0.into_xy();
    println!("Public key: x: {}, y: {}\n", pk_x, pk_y);

    let signed_fields = vec![
        ("type", vec![Transfer::TX_TYPE]),
        ("accountId", transfer.account_id.to_be_bytes().to_vec()),
        ("from", transfer.from.as_bytes().to_vec()),
        ("to", transfer.to.as_bytes().to_vec()),
        ("token", transfer.token.to_be_bytes().to_vec()),
        ("amount", pack_token_amount(&transfer.amount)),
        ("fee", pack_fee_amount(&transfer.fee)),
        ("nonce", transfer.nonce.to_be_bytes().to_vec()),
    ];
    println!("Signed transaction fields:");
    let mut field_concat = Vec::new();
    for (field, value) in signed_fields.into_iter() {
        println!("{}: 0x{}", field, hex::encode(&value));
        field_concat.extend(value.into_iter());
    }
    println!("Signed bytes: 0x{}", hex::encode(&field_concat));
    assert_eq!(
        field_concat,
        transfer.get_bytes(),
        "Protocol serialization mismatch"
    );
}

#[test]
fn test_print_withdraw_for_protocol() {
    let mut rng = XorShiftRng::from_seed([2, 2, 3, 4]);
    let key = gen_pk_and_msg().0;
    let withdraw = Withdraw::new_signed(
        gen_account_id(&mut rng),
        Address::from(rng.gen::<[u8; 20]>()),
        Address::from(rng.gen::<[u8; 20]>()),
        gen_token_id(&mut rng),
        BigUint::from(12_340_000_000_000u64),
        BigUint::from(56_700_000_000u64),
        rng.gen(),
        &key,
    )
    .expect("failed to sign withdraw");

    println!(
        "User representation:\n{}\n",
        serde_json::to_string_pretty(&withdraw).expect("json serialize")
    );

    println!("Signer:");
    println!("Private key: {}", key.0.to_string());
    let (pk_x, pk_y) = public_key_from_private(&key).0.into_xy();
    println!("Public key: x: {}, y: {}\n", pk_x, pk_y);

    let signed_fields = vec![
        ("type", vec![Withdraw::TX_TYPE]),
        ("accountId", withdraw.account_id.to_be_bytes().to_vec()),
        ("from", withdraw.from.as_bytes().to_vec()),
        ("to", withdraw.to.as_bytes().to_vec()),
        ("token", withdraw.token.to_be_bytes().to_vec()),
        (
            "fullAmount",
            withdraw.amount.to_u128().unwrap().to_be_bytes().to_vec(),
        ),
        ("fee", pack_fee_amount(&withdraw.fee)),
        ("nonce", withdraw.nonce.to_be_bytes().to_vec()),
    ];
    println!("Signed transaction fields:");
    let mut field_concat = Vec::new();
    for (field, value) in signed_fields.into_iter() {
        println!("{}: 0x{}", field, hex::encode(&value));
        field_concat.extend(value.into_iter());
    }
    println!("Signed bytes: 0x{}", hex::encode(&field_concat));
    assert_eq!(
        field_concat,
        withdraw.get_bytes(),
        "Protocol serialization mismatch"
    );
}

#[test]
fn test_musig_rescue_signing_verification() {
    let (pk, messages) = gen_pk_and_msg();

    for msg in &messages {
        let signature = TxSignature::sign_musig_rescue(&pk, msg);

        if let Some(sign_pub_key) = signature.verify_musig_rescue(msg) {
            let pub_key =
                PublicKey::from_private(&pk, FixedGenerators::SpendingKeyGenerator, &JUBJUB_PARAMS);
            assert!(
                sign_pub_key.0.eq(&pub_key.0),
                "Signature pub key is wrong, msg: {}",
                hex::encode(&msg)
            );
        } else {
            panic!("Signature is incorrect, msg: {}", hex::encode(&msg));
        }
    }
}

#[test]
fn test_ethereum_signature_verify_with_serialization() {
    let address: Address = "52312AD6f01657413b2eaE9287f6B9ADaD93D5FE".parse().unwrap();
    let message = "hello world";
    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestSignatureSerialize {
        signature: PackedEthSignature,
    }

    // signature calculated using ethers.js signer
    let test_signature_serialize = "{ \"signature\": \"0x111ea2824732851dd0893eaa5873597ba38ed08b69f6d8a0d7f5da810335566403d05281b1f56d12ca653e32eb7d67b76814b0cc8b0da2d7ad2c862d575329951b\"}";

    // test serialization
    let deserialized_signature: TestSignatureSerialize =
        serde_json::from_str(test_signature_serialize).expect("signature deserialize");
    let signature_after_roundtrip: TestSignatureSerialize = serde_json::from_str(
        &serde_json::to_string(&deserialized_signature).expect("signature serialize roundtrip"),
    )
    .expect("signature deserialize roundtrip");
    assert_eq!(
        deserialized_signature, signature_after_roundtrip,
        "signature serialize-deserialize roundtrip"
    );

    let recovered_address = deserialized_signature
        .signature
        .signature_recover_signer(&message.as_bytes())
        .expect("signature verification");

    assert_eq!(address, recovered_address, "recovered address mismatch");
}

#[test]
fn test_ethereum_signature_verify_examples() {
    // signatures created using geth
    // e.g. in geth console: eth.sign(eth.accounts[0], "0x")
    let examples = vec![
        ("0x8a91dc2d28b689474298d91899f0c1baf62cb85b", "0xdead", "0x13c34c76ffb42d97da67ddc5d275e92d758d1b48b5ee4b3bacd800cbeec3baff043a5ee63fea55485e1ee5d6f8b088daabd095f2ebbdc80a33806528b44bfccc1c"),
        // empty message
        ("0x8a91dc2d28b689474298d91899f0c1baf62cb85b", "0x", "0xd98f51c2ee0fd589e421348002dffec5d1b38e5bef9a41a699030456dc39298d12698158dc2a814b5f9ac6d433009dec87484a4579107be3f8f33907e92938291b"),
        // this example has v = 28, unlike others
        ("0x8a91dc2d28b689474298d91899f0c1baf62cb85b", "0x14", "0xd288b623af654c9d805e132812edf09ce244040376ca49112e91d437ecceed7c518690d4ae14149cd533f1ca4f081e6d2252c980fccc63de4d6bb818f1b668921c"),
        // same as first, but v is just recovery id
        ("0x8a91dc2d28b689474298d91899f0c1baf62cb85b", "0xdead", "0x13c34c76ffb42d97da67ddc5d275e92d758d1b48b5ee4b3bacd800cbeec3baff043a5ee63fea55485e1ee5d6f8b088daabd095f2ebbdc80a33806528b44bfccc01"),
    ];

    for (address, msg, signature) in examples {
        println!("addr: {}, msg: {}, sign: {}", address, msg, signature);
        let address = address[2..].parse::<Address>().unwrap();
        let msg = hex::decode(&msg[2..]).unwrap();

        let signature =
            PackedEthSignature::deserialize_packed(&hex::decode(&signature[2..]).unwrap())
                .expect("signature deserialize");
        let signer_address = signature
            .signature_recover_signer(&msg)
            .expect("signature verification");
        assert_eq!(address, signer_address, "signer address mismatch");
    }
}

#[test]
fn test_ethereum_signature_sign() {
    // data generated with `ethers.js`
    let private_key = "0b43c0f5b5a13a7047408d1f8c8ad32ba5879902ea6212184e0a5d1157281d76"
        .parse()
        .unwrap();

    let examples = vec![
        (b"hello world".to_vec(), "12c24491eefbac7e80f4d3f0400cd804667dab026fda1bc8bfe86650d872ba4215b0a0e297c48a54d9020daa3130222dadcb8f5ffdafc4b9293c3ef818b322b01c"),
        // empty message
        (Vec::new(), "8b7385c7bb8913b9fd176247efab0ccc72e3197abe8e2d4c6596ba58a32a91675f66e80560a5f1a42bd50d58da055630ac6c18875e5ba14a362e87e903f083941c"),
        // v = 27(others v = 28)
        (vec![0x12, 0x32, 0x12, 0x42], "463d955775a407eadfdb22437d53df42460977bf1c02cf830b579b6bd0000ff366e819af75fb7140e8797d56580acfcac0ad3567bbdeca118a5f5d37f09753f11b")
    ];
    for (msg, correct_signature) in examples {
        println!("message: 0x{}", hex::encode(&msg));
        let correct_signature = hex::decode(correct_signature).unwrap();
        let signature = PackedEthSignature::sign(&private_key, &msg)
            .expect("sign verify")
            .serialize_packed()
            .to_vec();
        assert_eq!(signature, correct_signature, "signature is incorrect");
    }
}
