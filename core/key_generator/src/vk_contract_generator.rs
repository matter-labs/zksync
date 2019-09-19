// Library to generate a EVM verifier contract

use bellman::groth16;
use pairing::{CurveAffine, Engine};

fn unpack_g1<E: Engine>(point: &E::G1Affine) -> Vec<String> {
    let uncompressed = point.into_uncompressed();
    let uncompressed_slice = uncompressed.as_ref();

    uncompressed_slice
        .chunks(32)
        .map(|c| "0x".to_owned() + &hex::encode(c))
        .collect()
}

fn unpack_g2<E: Engine>(point: &E::G2Affine) -> Vec<String> {
    let uncompressed = point.into_uncompressed();
    let uncompressed_slice = uncompressed.as_ref();
    uncompressed_slice
        .chunks(32)
        .map(|c| "0x".to_owned() + &hex::encode(c))
        .collect()
}

const SHIFT: &str = "        ";

fn render_array(name: &str, allocate: bool, values: &[Vec<String>]) -> String {
    let mut out = String::new();
    out.push('\n');
    let flattened: Vec<&String> = values.iter().flatten().collect();
    if allocate {
        out.push_str(&format!(
            "{}{} = new uint256[]({});\n",
            SHIFT,
            name,
            flattened.len()
        ));
    }
    for (i, s) in flattened.iter().enumerate() {
        out.push_str(&format!("{}{}[{}] = {};\n", SHIFT, name, i, s));
    }
    out
}

pub fn hardcode_vk<E: Engine>(vk: &groth16::VerifyingKey<E>) -> String {
    let mut out = String::new();

    let values = &[
        unpack_g1::<E>(&vk.alpha_g1),
        unpack_g2::<E>(&vk.beta_g2),
        unpack_g2::<E>(&vk.gamma_g2),
        unpack_g2::<E>(&vk.delta_g2),
    ];
    out.push_str(&render_array("vk", false, values));

    let ic: Vec<Vec<String>> = vk.ic.iter().map(unpack_g1::<E>).collect();
    out.push_str(&render_array("gammaABC", true, ic.as_slice()));

    out
}

pub fn generate_vk_contract<E: Engine>(
    vk: &groth16::VerifyingKey<E>,
    contract_name: String,
    function_name: String,
) -> String {
    format!(
        r#"
// This contract is generated programmatically

pragma solidity ^0.5.1;


// Hardcoded constants to avoid accessing store
contract {contract_name} {{

    function {function_name}() internal pure returns (uint256[14] memory vk, uint256[] memory gammaABC) {{

        {vk}

    }}

}}
"#,
        vk = hardcode_vk(&vk),
        contract_name = contract_name,
        function_name = function_name,
    )
}
