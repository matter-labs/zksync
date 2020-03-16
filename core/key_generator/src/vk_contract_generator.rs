// Library to generate a EVM verifier contract

use crypto_exports::bellman::groth16;
use crypto_exports::pairing::{CurveAffine, Engine};
use models::params::block_chunk_sizes;
use models::prover_utils::{
    get_block_proof_key_and_vk_path, get_exodus_proof_key_and_vk_path, get_keys_root_dir,
};

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

fn create_get_block_vk_function(block_sizes: &[usize]) -> String {
    let vk_selector_ifs = block_sizes
        .iter()
        .map(|size| {
            format!(
                "if (_chunks == uint32({block_size})) {{ return getVkBlock{block_size}(); }}\n",
                block_size = size
            )
        })
        .collect::<Vec<_>>();

    let mut vk_selector = String::new();

    for (if_number, if_block) in vk_selector_ifs.into_iter().enumerate() {
        if if_number != 0 {
            vk_selector.push_str("        else ")
        }
        vk_selector.push_str(&if_block);
    }
    format!(
        r#"
    function getVkBlock(uint32 _chunks) internal pure returns (uint256[14] memory vk, uint256[] memory gammaABC) {{
        {vk_selector}
    }}"#,
        vk_selector = vk_selector
    )
}

fn create_get_supported_block_sizes(block_sizes: &[usize]) -> String {
    let mut vk_selector_ifs = block_sizes
        .iter()
        .map(|size| {
            format!(
                "if (_size == uint32({block_size})) {{ return true; }}\n",
                block_size = size
            )
        })
        .collect::<Vec<_>>();
    vk_selector_ifs.push("{ return false; }".to_string());

    let mut result = String::new();

    for (if_number, if_block) in vk_selector_ifs.into_iter().enumerate() {
        if if_number != 0 {
            result.push_str("        else ")
        }
        result.push_str(&if_block);
    }

    format!(
        r#"
    function isBlockSizeSupported(uint32 _size) public pure returns (bool) {{
        {if_block}
    }}"#,
        if_block = result
    )
}

pub fn generate_vk_contract(
    contract_name: String,
    vk_functions: String,
    block_sizes: &[usize],
) -> String {
    format!(
        r#"
// This contract is generated programmatically

pragma solidity 0.5.16;


// Hardcoded constants to avoid accessing store
contract {contract_name} {{
    {vk_selector}
    {vk_functions}
    {supported_block_sizes}
}}"#,
        contract_name = contract_name,
        vk_selector = create_get_block_vk_function(block_sizes),
        vk_functions = vk_functions,
        supported_block_sizes = create_get_supported_block_sizes(block_sizes),
    )
}

pub fn generate_vk_function<E: Engine>(
    vk: &groth16::VerifyingKey<E>,
    function_name: &str,
) -> String {
    format!(
        r#"
    function {function_name}() internal pure returns (uint256[14] memory vk, uint256[] memory gammaABC) {{
    // This function is generated programmatically

        {vk}

    }}"#,
        vk = hardcode_vk(&vk),
        function_name = function_name,
    )
}

pub fn compose_verifer_keys_contract() {
    let mut contract_file_path = get_keys_root_dir();
    contract_file_path.push("VerificationKey.sol");

    let mut vk_functions = Vec::new();

    let exit_key_vk = std::fs::read_to_string(get_exodus_proof_key_and_vk_path().1)
        .expect("Fail to read exit key vk");
    vk_functions.push(exit_key_vk);

    for &block_size in block_chunk_sizes() {
        let block_proof_vk = std::fs::read_to_string(get_block_proof_key_and_vk_path(block_size).1)
            .expect("Fail to read block proof vk");
        vk_functions.push(block_proof_vk);
    }

    let contract_content = generate_vk_contract(
        "VerificationKey".to_string(),
        vk_functions.concat(),
        block_chunk_sizes(),
    );

    std::fs::write(contract_file_path, contract_content)
        .expect("Failed to create verifier key contract");
}
