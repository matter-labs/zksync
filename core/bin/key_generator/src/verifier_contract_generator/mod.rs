//! Library to generate a EVM verifier contract

use std::collections::HashMap;
use std::path::PathBuf;

use handlebars::to_json;
use handlebars::Handlebars;

use crate::verifier_contract_generator::render_vk::{
    get_exit_vk_tree_root_hash, get_vk_tree_root_hash, rendered_key,
};
use zksync_config::AvailableBlockSizesConfig;
use zksync_prover_utils::fs_utils::{
    get_recursive_verification_key_path, get_verifier_contract_key_path,
};
use zksync_utils::parse_env;

mod render_vk;

/// Creates verifier contract compatible with our main contract using generated verification keys.
/// Contract is created from the template using `handlebars` crate.
pub(crate) fn create_verifier_contract(config: AvailableBlockSizesConfig) {
    let template = &std::fs::read_to_string(get_verifier_template_file())
        .expect("failed to read Verifier template file");
    let mut template_params = HashMap::new();

    template_params.insert(
        "vk_tree_root".to_string(),
        to_json(get_vk_tree_root_hash(&config.blocks_chunks)),
    );

    template_params.insert(
        "vk_tree_root_exit".to_string(),
        to_json(get_exit_vk_tree_root_hash()),
    );

    template_params.insert(
        "vk_max_index".to_string(),
        to_json(config.blocks_chunks.len() - 1),
    );

    let chunks = to_json(config.blocks_chunks);
    template_params.insert("chunks".to_string(), chunks);

    let sizes = to_json(config.aggregated_proof_sizes.clone());
    template_params.insert("sizes".to_string(), sizes);

    let templates_for_key_getters = config
        .aggregated_proof_sizes
        .into_iter()
        .map(|blocks| {
            let key_getter_name = format!("getVkAggregated{}", blocks);
            let verification_key_path = get_recursive_verification_key_path(blocks);
            rendered_key(&key_getter_name, verification_key_path)
        })
        .collect::<Vec<_>>();
    template_params.insert("keys".to_string(), to_json(templates_for_key_getters));

    let res = Handlebars::new()
        .render_template(template, &template_params)
        .expect("failed to render Verifiers.sol template");
    std::fs::write(get_verifier_contract_key_path(), res).expect("failed to wrtie Verifier.sol");
    log::info!("Verifier contract successfully generated");
}

fn get_verifier_template_file() -> PathBuf {
    let mut contract = parse_env::<PathBuf>("ZKSYNC_HOME");
    contract.push("core");
    contract.push("bin");
    contract.push("key_generator");
    contract.push("src");
    contract.push("verifier_contract_generator");
    contract.push("VerifierTemplate.sol");
    contract
}
