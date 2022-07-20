use crate::eip712_signature::{EIP712TypedStructure, Eip712Domain};
use serde_json::{Map, Value};

/// Formats the data that needs to be signed in json according to the standard eip-712.
/// Compatible with `eth_signTypedData` RPC call.
pub fn get_eip712_json<T: EIP712TypedStructure>(
    eip712_domain: &Eip712Domain,
    typed_struct: &T,
) -> Value {
    let types = {
        let mut res = Map::new();

        let mut vec_types = eip712_domain.get_json_types();
        vec_types.append(&mut typed_struct.get_json_types());

        for mut member_type in vec_types {
            if let Some(member_type) = member_type.as_object_mut() {
                res.append(member_type);
            }
        }
        res
    };

    serde_json::json!({
        "primaryType": T::TYPE_NAME,
        "domain": serde_json::to_value(eip712_domain).expect("serialization fail"),
        "message": serde_json::to_value(typed_struct).expect("serialization fail"),
        "types": serde_json::to_value(types).expect("serialization fail"),
    })
}
