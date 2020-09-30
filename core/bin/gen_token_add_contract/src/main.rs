//! This executable is used to generate TokenDeployInit contract for DeployFactory.
//!
//! TokenDeployInit contract contains list of tokens that we add to the Governance contract when
//! we first deploy it.
//!
//! List of tokens for initial deployment depends on current `ETH_NETWORK` and is available in the
//! `etc/tokens/${ETH_NETWORK}.json`
//!
//! For localhost network we deploy test ERC20 token and generate token list file in the `zksync init` script.

use handlebars::{to_json, Handlebars};
use std::collections::HashMap;
use std::fs::{read_to_string, write};
use std::path::PathBuf;
use zksync_types::tokens::get_genesis_token_list;
use zksync_utils::{get_env, parse_env};

fn main() {
    let network = get_env("ETH_NETWORK");

    let token_list = get_genesis_token_list(&network).expect("Initial tokens list not found");
    let template = &read_to_string(get_token_add_template_file())
        .expect("failed to read TokenInit template file");

    let mut template_params = HashMap::new();
    template_params.insert("token_len", to_json(token_list.len()));
    template_params.insert("tokens", to_json(token_list));

    let contract = Handlebars::new()
        .render_template(template, &template_params)
        .expect("failed to render Verifiers.sol template");
    write(get_token_init_output_path(), contract).expect("failed to write TokenInit contract");
}

fn get_token_add_template_file() -> PathBuf {
    let mut template = parse_env::<PathBuf>("ZKSYNC_HOME");
    template.push("core");
    template.push("bin");
    template.push("gen_token_add_contract");
    template.push("src");
    template.push("TokenInitTemplate.sol");
    template
}

fn get_token_init_output_path() -> PathBuf {
    let mut contract = parse_env::<PathBuf>("ZKSYNC_HOME");
    contract.push("contracts");
    contract.push("contracts");
    contract.push("TokenInit");
    contract.set_extension("sol");
    contract
}
