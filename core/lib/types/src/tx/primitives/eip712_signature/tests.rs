use crate::eip712_signature::{
    struct_builder::StructBuilder,
    typed_structure::{EIP712TypedStructure, Eip712Domain},
    utils::get_eip712_json,
};
use crate::tx::PackedEthSignature;
use parity_crypto::Keccak256;
use serde::Serialize;
use std::str::FromStr;
use zksync_basic_types::{Address, H256, U256};

#[derive(Clone, Serialize)]
struct Person {
    name: String,
    wallet: Address,
}

impl EIP712TypedStructure for Person {
    const TYPE_NAME: &'static str = "Person";

    fn build_structure<BUILDER: StructBuilder>(&self, builder: &mut BUILDER) {
        builder.add_member("name", &self.name);
        builder.add_member("wallet", &self.wallet);
    }
}

#[derive(Clone, Serialize)]
struct Mail {
    from: Person,
    to: Person,
    contents: String,
    data: [u8; 20],
}

impl EIP712TypedStructure for Mail {
    const TYPE_NAME: &'static str = "Mail";
    fn build_structure<BUILDER: StructBuilder>(&self, builder: &mut BUILDER) {
        builder.add_member("from", &self.from);
        builder.add_member("to", &self.to);
        builder.add_member("contents", &self.contents);
        builder.add_member("data", &self.data);
    }
}

#[test]
fn test_encode_eip712_typed_struct() {
    let domain = Eip712Domain {
        name: "Ether Mail".to_owned(),
        version: "1".to_owned(),
        chain_id: U256::from(1u8),
    };

    let message = Mail {
        from: Person {
            name: "Cow".to_owned(),
            wallet: Address::from_str("CD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826").unwrap(),
        },
        to: Person {
            name: "Bob".to_owned(),
            wallet: Address::from_str("bBbBBBBbbBBBbbbBbbBbbbbBBbBbbbbBbBbbBBbB").unwrap(),
        },
        contents: "Hello, Bob!".to_string(),
        data: [1, 2, 3, 4, 5, 1, 2, 3, 4, 5, 1, 2, 3, 4, 5, 1, 2, 3, 4, 5],
    };

    assert_eq!(
        &message.encode_type(),
        "Mail(Person from,Person to,string contents,bytes20 data)Person(string name,address wallet)"
    );

    assert_eq!(
        &message.encode_data()[..],
        [
            H256::from_str("fc71e5fa27ff56c350aa531bc129ebdf613b772b6604664f5d8dbe21b85eb0c8")
                .unwrap(),
            H256::from_str("cd54f074a4af31b4411ff6a60c9719dbd559c221c8ac3492d9d872b041d703d1")
                .unwrap(),
            H256::from_str("b5aadf3154a261abdd9086fc627b61efca26ae5702701d05cd2305f7c52a2fc8")
                .unwrap(),
            H256::from_str("0102030405010203040501020304050102030405000000000000000000000000")
                .unwrap()
        ]
    );

    assert_eq!(
        message.hash_struct(),
        H256::from_str("be9b2f924d8a769bf1dfffbebf79bacacd82aa65f767afcbf6f363e456d02de9").unwrap()
    );

    assert_eq!(
        &domain.encode_type(),
        "EIP712Domain(string name,string version,uint256 chainId)"
    );

    assert_eq!(
        &domain.encode_data()[..],
        [
            H256::from_str("c70ef06638535b4881fafcac8287e210e3769ff1a8e91f1b95d6246e61e4d3c6")
                .unwrap(),
            H256::from_str("c89efdaa54c0f20c7adf612882df0950f5a951637e0307cdcb4c672f298b8bc6")
                .unwrap(),
            H256::from_str("0000000000000000000000000000000000000000000000000000000000000001")
                .unwrap(),
        ]
    );

    assert_eq!(
        domain.hash_struct(),
        H256::from_str("3b98b16ad068d9d8854a6a416bd476de44a4933ec5104d7c786a422ab262ed14").unwrap()
    );

    let private_key = b"cow".keccak256().into();
    let address_owner = PackedEthSignature::address_from_private_key(&private_key).unwrap();

    let signature = PackedEthSignature::sign_typed_data(&private_key, &domain, &message).unwrap();
    let signed_bytes = PackedEthSignature::typed_data_to_signed_bytes(&domain, &message);

    assert_eq!(
        address_owner,
        signature
            .signature_recover_signer_from_hash(signed_bytes)
            .unwrap()
    );
}

#[test]
fn test_get_eip712_json() {
    let domain = Eip712Domain {
        name: "Ether Mail".to_owned(),
        version: "1".to_owned(),
        chain_id: U256::from(1u8),
    };

    let message = Mail {
        from: Person {
            name: "Cow".to_owned(),
            wallet: Address::from_str("d94e3dc39d4cad1dad634e7eb585a57a19dc7efe").unwrap(),
        },
        to: Person {
            name: "Bob".to_owned(),
            wallet: Address::from_str("d94e3dc39d4cad1dad634e7eb585a57a19dc7efe").unwrap(),
        },
        contents: "Hello, Bob!".to_string(),
        data: [1, 2, 3, 4, 5, 1, 2, 3, 4, 5, 1, 2, 3, 4, 5, 1, 2, 3, 4, 5],
    };

    let expected_value = r#"{
        "domain":{
           "chainId":"0x1",
           "name":"Ether Mail",
           "version":"1"
        },
        "message":{
           "contents":"Hello, Bob!",
           "from":{
              "name":"Cow",
              "wallet":"0xd94e3dc39d4cad1dad634e7eb585a57a19dc7efe"
           },
           "to":{
              "name":"Bob",
              "wallet":"0xd94e3dc39d4cad1dad634e7eb585a57a19dc7efe"
           },
           "data": [1, 2, 3, 4, 5, 1, 2, 3, 4, 5, 1, 2, 3, 4, 5, 1, 2, 3, 4, 5]
        },
        "primaryType":"Mail",
        "types":{
           "EIP712Domain":[
              {
                 "name":"name",
                 "type":"string"
              },
              {
                 "name":"version",
                 "type":"string"
              },
              {
                 "name":"chainId",
                 "type":"uint256"
              }
           ],
           "Mail":[
              {
                 "name":"from",
                 "type":"Person"
              },
              {
                 "name":"to",
                 "type":"Person"
              },
              {
                 "name":"contents",
                 "type":"string"
              },
              {
                 "name":"data",
                 "type":"bytes20"
              }
           ],
           "Person":[
              {
                 "name":"name",
                 "type":"string"
              },
              {
                 "name":"wallet",
                 "type":"address"
              }
           ]
        }
     }"#;

    assert_eq!(
        get_eip712_json(&domain, &message),
        serde_json::from_str::<serde_json::Value>(expected_value).unwrap()
    );
}
