// Library to generate a EVM verifier contract

use bellman::{Circuit, ConstraintSystem, SynthesisError};
use ff::{Field, PrimeField};
use pairing::{Engine, CurveAffine, EncodedPoint};
use bellman::groth16;
use pairing::bn256::{Bn256, Fr};
use std::fmt;

pub fn generate_vk_contract<E: Engine>(vk: &groth16::VerifyingKey<E>) -> String {
    format!(
r#"
contract DemoVerifier is Verifier {{

    function getVk() internal view returns (uint256[14] memory vk, uint256[] memory gammaABC) {{
        {vk}
    }}

}}
"#,
    vk = hardcode_vk(&vk))
}

fn unpack<T: CurveAffine>(t: &T) -> Vec<String>
{
    t.into_uncompressed().as_ref().chunks(32).map(|c| "0x".to_owned() + &hex::encode(c)).collect()
}

const SHIFT: &str = "        ";

fn render_array(name: &str, allocate: bool, values: &[Vec<String>]) -> String {
    let mut out = String::new();
    out.push('\n');
    let flattened: Vec<&String> = values.into_iter().flatten().collect();
    if allocate {
        out.push_str(&format!("{}{} = new uint256[]({});\n", SHIFT, name, flattened.len()));
    }
    for (i, s) in flattened.iter().enumerate() {
        out.push_str(&format!("{}{}[{}] = {};\n", SHIFT, name, i, s));
    }
    out
}

fn hardcode_vk<E: Engine>(vk: &groth16::VerifyingKey<E>) -> String {
    let mut out = String::new();

    let values = &[
        unpack(&vk.alpha_g1),
        unpack(&vk.beta_g2),
        unpack(&vk.gamma_g2),
        unpack(&vk.delta_g2),
    ];
    out.push_str(&render_array("vk", false, values));

    let ic: Vec<Vec<String>> = vk.ic.iter().map(unpack).collect();
    out.push_str(&render_array("gammaABC", true, ic.as_slice()));

    out
}