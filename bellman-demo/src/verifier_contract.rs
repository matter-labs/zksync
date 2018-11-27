// Library to generate a demo EVM verifier contract

use bellman::{Circuit, ConstraintSystem, SynthesisError};
use ff::{Field, PrimeField};
use pairing::{Engine, CurveAffine, EncodedPoint};
use bellman::groth16;
use pairing::bn256::{Bn256, Fr};
use std::fmt;

pub fn generate_demo_contract<E: Engine>(vk: &groth16::VerifyingKey<E>, proof: &groth16::Proof<E>, inputs: &[E::Fr], inputs_extra: &str) -> String {
    format!("{}{}", STANDARD_VERIFIER, demo_verifier(vk, proof, inputs, inputs_extra))
}

const STANDARD_VERIFIER: &str =
r#"pragma solidity ^0.4.24;

// from https://github.com/HarryR/ethsnarks/blob/master/contracts/Verifier.sol
contract Verifier {

    function NegateY( uint256 Y ) internal pure returns (uint256) {
        uint q = 21888242871839275222246405745257275088696311157297823662689037894645226208583;
        return q - (Y % q);
    }

    function Verify ( uint256[14] in_vk, uint256[] vk_gammaABC, uint256[8] in_proof, uint256[] proof_inputs ) internal view returns (bool) {
        require( ((vk_gammaABC.length / 2) - 1) == proof_inputs.length );

        // Compute the linear combination vk_x
        uint256[3] memory mul_input;
        uint256[4] memory add_input;
        bool success;
        uint m = 2;

        // First two fields are used as the sum
        add_input[0] = vk_gammaABC[0];
        add_input[1] = vk_gammaABC[1];

        // Performs a sum of gammaABC[0] + sum[ gammaABC[i+1]^proof_inputs[i] ]
        for (uint i = 0; i < proof_inputs.length; i++) {
            mul_input[0] = vk_gammaABC[m++];
            mul_input[1] = vk_gammaABC[m++];
            mul_input[2] = proof_inputs[i];

            assembly {
                // ECMUL, output to last 2 elements of `add_input`
                success := staticcall(sub(gas, 2000), 7, mul_input, 0x80, add(add_input, 0x40), 0x60)
            }
            require( success );

            assembly {
                // ECADD
                success := staticcall(sub(gas, 2000), 6, add_input, 0xc0, add_input, 0x60)
            }
            require( success );
        }

        uint[24] memory input = [
            // (proof.A, proof.B)
            in_proof[0], in_proof[1],                           // proof.A   (G1)
            in_proof[2], in_proof[3], in_proof[4], in_proof[5], // proof.B   (G2)

            // (-vk.alpha, vk.beta)
            in_vk[0], NegateY(in_vk[1]),                        // -vk.alpha (G1)
            in_vk[2], in_vk[3], in_vk[4], in_vk[5],             // vk.beta   (G2)

            // (-vk_x, vk.gamma)
            add_input[0], NegateY(add_input[1]),                // -vk_x     (G1)
            in_vk[6], in_vk[7], in_vk[8], in_vk[9],             // vk.gamma  (G2)

            // (-proof.C, vk.delta)
            in_proof[6], NegateY(in_proof[7]),                  // -proof.C  (G1)
            in_vk[10], in_vk[11], in_vk[12], in_vk[13]          // vk.delta  (G2)
        ];

        uint[1] memory out;
        assembly {
            success := staticcall(sub(gas, 2000), 8, input, 768, out, 0x20)
        }
        require(success);
        return out[0] != 0;
    }
}
"#;

fn demo_verifier<E: Engine>(vk: &groth16::VerifyingKey<E>, proof: &groth16::Proof<E>, inputs: &[E::Fr], inputs_extra: &str) -> String {
    format!(
r#"
contract DemoVerifier is Verifier {{

    function getVk() internal view returns (uint256[14] memory vk, uint256[] memory gammaABC) {{
        {vk}
    }}

    function getProof() internal view returns (uint256[8] memory proof) {{
        {proof}
    }}

    function getInputs() internal view returns (uint256[] memory inputs) {{
        {inputs}
        {inputs_extra}
    }}

    function verify( ) public view returns (bool) {{
        var (vk, gammaABC) = getVk();
        return Verify(vk, gammaABC, getProof(), getInputs());
    }}
}}
"#,
    vk = hardcode_vk(vk),
    proof = hardcode_proof(proof),
    inputs = hardcode_inputs::<E>(inputs),
    inputs_extra = inputs_extra)
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

fn hardcode_proof<E: Engine>(proof: &groth16::Proof<E>) -> String {
    let values = &[
        unpack(&proof.a),
        unpack(&proof.b),
        unpack(&proof.c),
    ];
    render_array("proof", false, values)
}

fn hardcode_inputs<E: Engine>(inputs: &[E::Fr]) -> String {
    let values: Vec<Vec<String>> = inputs.iter().map(|i| {vec!(format!("{}", inputs[0].into_repr()))}).collect();
    render_array("inputs", true, values.as_slice())
}
