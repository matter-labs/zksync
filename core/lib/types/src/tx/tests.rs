use num::{BigUint, ToPrimitive};
use serde::{Deserialize, Serialize};
use zksync_basic_types::Address;
use zksync_crypto::{
    franklin_crypto::{
        eddsa::{PrivateKey, PublicKey},
        jubjub::FixedGenerators,
    },
    pairing::bn256::Bn256,
    params::{max_account_id, max_fungible_token_id, CURRENT_TX_VERSION, JUBJUB_PARAMS},
    primitives::rescue_hash_orders,
    public_key_from_private,
    rand::{Rng, SeedableRng, XorShiftRng},
};

use super::*;
use crate::{
    helpers::{pack_fee_amount, pack_token_amount},
    AccountId, Engine, Nonce, PubKeyHash, TokenId, H256,
};

fn gen_pk_and_msg() -> (PrivateKey<Engine>, Vec<Vec<u8>>) {
    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);

    let pk = PrivateKey(rng.gen());

    let messages = vec![Vec::<u8>::new(), b"hello world".to_vec()];

    (pk, messages)
}

fn gen_account_id<T: Rng>(rng: &mut T) -> AccountId {
    AccountId(rng.gen::<u32>().min(*max_account_id()))
}

fn gen_token_id<T: Rng>(rng: &mut T) -> TokenId {
    TokenId(rng.gen::<u32>().min(*max_fungible_token_id()))
}

fn gen_nft_token_id<T: Rng>(rng: &mut T) -> TokenId {
    TokenId(rng.gen::<u32>().max(*max_fungible_token_id() + 1))
}

#[test]
fn test_print_swap_for_protocol() {
    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);
    let key_0 = gen_pk_and_msg().0;
    let key_1 = gen_pk_and_msg().0;
    let key_2 = gen_pk_and_msg().0;
    let token_a = gen_token_id(&mut rng);
    let token_b = gen_token_id(&mut rng);

    let order_0 = Order::new_signed(
        gen_account_id(&mut rng),
        Address::from(rng.gen::<[u8; 20]>()),
        Nonce(rng.gen()),
        token_a,
        token_b,
        (BigUint::from(12u8), BigUint::from(18u8)),
        BigUint::from(12_000_000_000u64),
        Default::default(),
        &key_0,
    )
    .expect("failed to sign order");

    let order_1 = Order::new_signed(
        gen_account_id(&mut rng),
        Address::from(rng.gen::<[u8; 20]>()),
        Nonce(rng.gen()),
        token_b,
        token_a,
        (BigUint::from(18u8), BigUint::from(12u8)),
        BigUint::from(18_000_000_000u64),
        Default::default(),
        &key_1,
    )
    .expect("failed to sign order");

    let swap = Swap::new_signed(
        gen_account_id(&mut rng),
        Address::from(rng.gen::<[u8; 20]>()),
        Nonce(rng.gen()),
        (order_0.clone(), order_1.clone()),
        (
            BigUint::from(12_000_000_000u64),
            BigUint::from(18_000_000_000u64),
        ),
        BigUint::from(56_000_000u64),
        gen_token_id(&mut rng),
        &key_2,
    )
    .expect("failed to sign transfer");

    println!(
        "User representation:\n{}\n",
        serde_json::to_string_pretty(&swap).expect("json serialize")
    );

    let print_signer = |name, key: PrivateKey<Bn256>| {
        println!("Signer ({}):", name);
        println!("Private key: {}", key.0);
        let (pk_x, pk_y) = public_key_from_private(&key).0.into_xy();
        println!("Public key: x: {}, y: {}\n", pk_x, pk_y);
    };

    print_signer("account_a", key_0);
    print_signer("account_b", key_1);
    print_signer("submitter", key_2);

    let mut orders_bytes = Vec::new();
    orders_bytes.extend(order_0.get_bytes());
    orders_bytes.extend(order_1.get_bytes());

    let signed_fields = vec![
        ("type", vec![255u8 - Swap::TX_TYPE]),
        ("version", vec![CURRENT_TX_VERSION]),
        ("submitterId", swap.submitter_id.to_be_bytes().to_vec()),
        (
            "submitterAddress",
            swap.submitter_address.as_bytes().to_vec(),
        ),
        ("nonce", swap.nonce.to_be_bytes().to_vec()),
        ("orders_hash", rescue_hash_orders(&orders_bytes)),
        ("fee_token", swap.fee_token.to_be_bytes().to_vec()),
        ("fee", pack_fee_amount(&swap.fee)),
        ("amounts[0]", pack_token_amount(&swap.amounts.0)),
        ("amounts[1]", pack_token_amount(&swap.amounts.1)),
    ];
    println!("Signed transaction fields:");
    let mut field_concat = Vec::new();
    for (field, value) in signed_fields.into_iter() {
        println!("{}: 0x{}", field, hex::encode(&value));
        field_concat.extend(value);
    }
    println!("Signed bytes: 0x{}", hex::encode(&field_concat));
    assert_eq!(
        field_concat,
        swap.get_sign_bytes(),
        "Protocol serialization mismatch"
    );
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
        Nonce(rng.gen()),
        Default::default(),
        &key,
    )
    .expect("failed to sign transfer");

    println!(
        "User representation:\n{}\n",
        serde_json::to_string_pretty(&transfer).expect("json serialize")
    );

    println!("Signer:");
    println!("Private key: {}", key.0);
    let (pk_x, pk_y) = public_key_from_private(&key).0.into_xy();
    println!("Public key: x: {}, y: {}\n", pk_x, pk_y);

    let signed_fields = vec![
        ("type", vec![255u8 - Transfer::TX_TYPE]),
        ("version", vec![CURRENT_TX_VERSION]),
        ("accountId", transfer.account_id.to_be_bytes().to_vec()),
        ("from", transfer.from.as_bytes().to_vec()),
        ("to", transfer.to.as_bytes().to_vec()),
        ("token", transfer.token.to_be_bytes().to_vec()),
        ("amount", pack_token_amount(&transfer.amount)),
        ("fee", pack_fee_amount(&transfer.fee)),
        ("nonce", transfer.nonce.to_be_bytes().to_vec()),
        (
            "time_range",
            transfer
                .time_range
                .expect("no time range on transfer")
                .as_be_bytes()
                .to_vec(),
        ),
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
fn test_print_change_pub_key_for_protocol() {
    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);
    let key = gen_pk_and_msg().0;
    PubKeyHash::from_privkey(&key);
    let transfer = ChangePubKey::new_signed(
        gen_account_id(&mut rng),
        Address::from(rng.gen::<[u8; 20]>()),
        PubKeyHash::from_privkey(&key),
        gen_token_id(&mut rng),
        BigUint::from(56_700_000_000u64),
        Nonce(rng.gen()),
        Default::default(),
        None,
        &key,
        None,
    )
    .expect("failed to sign transfer");

    println!(
        "User representation:\n{}\n",
        serde_json::to_string_pretty(&transfer).expect("json serialize")
    );

    println!("Signer:");
    println!("Private key: {}", key.0);
    let (pk_x, pk_y) = public_key_from_private(&key).0.into_xy();
    println!("Public key: x: {}, y: {}\n", pk_x, pk_y);

    let signed_fields = vec![
        ("type", vec![255u8 - ChangePubKey::TX_TYPE]),
        ("version", vec![CURRENT_TX_VERSION]),
        ("accountId", transfer.account_id.to_be_bytes().to_vec()),
        ("account", transfer.account.as_bytes().to_vec()),
        ("new_pub_key_hash", transfer.new_pk_hash.data.to_vec()),
        ("token", transfer.fee_token.to_be_bytes().to_vec()),
        ("fee", pack_fee_amount(&transfer.fee)),
        ("nonce", transfer.nonce.to_be_bytes().to_vec()),
        (
            "time_range",
            transfer
                .time_range
                .expect("no time range on transfer")
                .as_be_bytes()
                .to_vec(),
        ),
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
        Nonce(rng.gen()),
        Default::default(),
        &key,
    )
    .expect("failed to sign withdraw");

    println!(
        "User representation:\n{}\n",
        serde_json::to_string_pretty(&withdraw).expect("json serialize")
    );

    println!("Signer:");
    println!("Private key: {}", key.0);
    let (pk_x, pk_y) = public_key_from_private(&key).0.into_xy();
    println!("Public key: x: {}, y: {}\n", pk_x, pk_y);

    let signed_fields = vec![
        ("type", vec![255u8 - Withdraw::TX_TYPE]),
        ("version", vec![CURRENT_TX_VERSION]),
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
        (
            "time_range",
            withdraw
                .time_range
                .expect("no time range on withdraw")
                .as_be_bytes()
                .to_vec(),
        ),
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
fn test_print_withdraw_nft_for_protocol() {
    let mut rng = XorShiftRng::from_seed([2, 2, 3, 4]);
    let key = gen_pk_and_msg().0;
    let withdraw = WithdrawNFT::new_signed(
        gen_account_id(&mut rng),
        Address::from(rng.gen::<[u8; 20]>()),
        Address::from(rng.gen::<[u8; 20]>()),
        gen_nft_token_id(&mut rng),
        gen_token_id(&mut rng),
        BigUint::from(12_340_000_000_000u64),
        Nonce(rng.gen()),
        Default::default(),
        &key,
    )
    .expect("failed to sign withdraw");

    println!(
        "User representation:\n{}\n",
        serde_json::to_string_pretty(&withdraw).expect("json serialize")
    );

    println!("Signer:");
    println!("Private key: {}", key.0);
    let (pk_x, pk_y) = public_key_from_private(&key).0.into_xy();
    println!("Public key: x: {}, y: {}\n", pk_x, pk_y);

    let signed_fields = vec![
        ("type", vec![255u8 - WithdrawNFT::TX_TYPE]),
        ("version", vec![CURRENT_TX_VERSION]),
        ("accountId", withdraw.account_id.to_be_bytes().to_vec()),
        ("from", withdraw.from.as_bytes().to_vec()),
        ("to", withdraw.to.as_bytes().to_vec()),
        ("token", withdraw.token.to_be_bytes().to_vec()),
        ("fee_token", withdraw.fee_token.to_be_bytes().to_vec()),
        ("fee", pack_fee_amount(&withdraw.fee)),
        ("nonce", withdraw.nonce.to_be_bytes().to_vec()),
        ("time_range", withdraw.time_range.as_be_bytes().to_vec()),
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
fn test_print_mint_nft_for_protocol() {
    let mut rng = XorShiftRng::from_seed([2, 2, 3, 4]);
    let key = gen_pk_and_msg().0;
    let mint_nft = MintNFT::new_signed(
        gen_account_id(&mut rng),
        Address::from(rng.gen::<[u8; 20]>()),
        H256::random(),
        Address::from(rng.gen::<[u8; 20]>()),
        BigUint::from(12_340_000_000_000u64),
        gen_token_id(&mut rng),
        Nonce(rng.gen()),
        &key,
    )
    .expect("failed to sign withdraw");

    println!(
        "User representation:\n{}\n",
        serde_json::to_string_pretty(&mint_nft).expect("json serialize")
    );

    println!("Signer:");
    println!("Private key: {}", key.0);
    let (pk_x, pk_y) = public_key_from_private(&key).0.into_xy();
    println!("Public key: x: {}, y: {}\n", pk_x, pk_y);

    let signed_fields = vec![
        ("type", vec![255u8 - MintNFT::TX_TYPE]),
        ("version", vec![CURRENT_TX_VERSION]),
        ("creatorId", mint_nft.creator_id.to_be_bytes().to_vec()),
        (
            "creatorAddress",
            mint_nft.creator_address.as_bytes().to_vec(),
        ),
        ("contentHash", mint_nft.content_hash.as_bytes().to_vec()),
        ("recipient", mint_nft.recipient.as_bytes().to_vec()),
        ("fee_token", mint_nft.fee_token.to_be_bytes().to_vec()),
        ("fee", pack_fee_amount(&mint_nft.fee)),
        ("nonce", mint_nft.nonce.to_be_bytes().to_vec()),
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
        mint_nft.get_bytes(),
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
                hex::encode(msg)
            );
        } else {
            panic!("Signature is incorrect, msg: {}", hex::encode(msg));
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
        .signature_recover_signer_from_raw_message(message.as_bytes())
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
            .signature_recover_signer_from_raw_message(&msg)
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

/// Checks that we are able to decode old entries from the database.
#[test]
fn eth_sign_data_compatibility() {
    // Messages were stored as strings rather than byte vectors.
    #[derive(Clone, Serialize)]
    struct OldEthSignData {
        pub signature: TxEthSignature,
        pub message: String,
    }
    // Generate dummy signature.
    let private_key = "0b43c0f5b5a13a7047408d1f8c8ad32ba5879902ea6212184e0a5d1157281d76"
        .parse()
        .unwrap();
    let message = "Sample text".to_owned();
    let signature = TxEthSignature::EthereumSignature(
        PackedEthSignature::sign(&private_key, message.as_bytes()).unwrap(),
    );

    let old_eth_sign_data = OldEthSignData { signature, message };
    let value = serde_json::to_value(old_eth_sign_data.clone()).unwrap();

    let eth_sign_data: EthSignData =
        serde_json::from_value(value).expect("failed to decode old message format");

    assert_eq!(old_eth_sign_data.signature, eth_sign_data.signature);
    assert_eq!(
        old_eth_sign_data.message.as_bytes(),
        eth_sign_data.message.as_slice()
    );
    // We are able to encode/decode messages in new format.
    let value = serde_json::to_value(eth_sign_data.clone()).unwrap();
    let deserialized: EthSignData =
        serde_json::from_value(value).expect("failed to decode EthSignData");

    assert_eq!(deserialized.signature, eth_sign_data.signature);
    assert_eq!(deserialized.message, eth_sign_data.message);
}

#[test]
fn test_check_signature() {
    let (pk, msg) = gen_pk_and_msg();
    let signature = TxSignature::sign_musig(&pk, &msg[1])
        .signature
        .serialize_packed()
        .unwrap();

    assert_eq!(hex::encode(signature), "4e3298ac8cc13868dbbc94ad6fb41085ffe05b3c2eee22f88b05e69b7a5126aea723d7a3e7282ef5a32d9479c9c8dde52b3e3c462dd445dcd8158ebb6edb6000");
}
