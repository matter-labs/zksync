use std::fs::File;
use zksync_config::ChainConfig;
use zksync_crypto::params::{RECURSIVE_CIRCUIT_NUM_INPUTS, RECURSIVE_CIRCUIT_VK_TREE_DEPTH};
use zksync_crypto::recursive_aggregation_circuit;
use zksync_prover_utils::fs_utils::{
    get_recursive_verification_key_path, get_universal_setup_monomial_form,
};

pub fn make_recursive_verification_keys(config: ChainConfig) {
    for (proofs, setup_power) in config
        .circuit
        .supported_aggregated_proof_sizes_with_setup_pow()
    {
        let path = get_recursive_verification_key_path(proofs);
        vlog::info!(
            "Generating recursive verification key for {} proofs into: {}",
            proofs,
            path.display()
        );
        assert!(
            !path.exists(),
            "path for saving verification key exists: {}",
            path.display()
        );
        let vk_file = File::create(&path).expect("can't create file at verification key path");
        let (vk, _) =
            recursive_aggregation_circuit::circuit::create_recursive_circuit_vk_and_setup(
                proofs,
                RECURSIVE_CIRCUIT_NUM_INPUTS,
                RECURSIVE_CIRCUIT_VK_TREE_DEPTH,
                &get_universal_setup_monomial_form(setup_power).expect("Universal setup no found"),
            )
            .expect("Failed to generate recursive circuit verification keys");
        vk.write(vk_file).expect("Failed to save verification key");
    }
}

pub fn count_gates_recursive_verification_keys() {
    fn get_setup_size(proofs: usize) -> u32 {
        let recursive_setup =
            recursive_aggregation_circuit::circuit::create_recursive_circuit_setup(
                proofs,
                RECURSIVE_CIRCUIT_NUM_INPUTS,
                RECURSIVE_CIRCUIT_VK_TREE_DEPTH,
            )
            .expect("Failed to generate recursive circuit setup");
        recursive_setup.n.next_power_of_two().trailing_zeros()
    }

    let mut proofs = 1;
    let mut setup_power = get_setup_size(proofs);
    while setup_power <= 26 {
        let new_proofs = proofs + 1;
        let new_setup_power = get_setup_size(new_proofs);
        if new_setup_power <= setup_power {
            proofs = new_proofs;
        } else {
            vlog::info!("setup_size_log2: {}, proofs: {}", setup_power, proofs);
            proofs *= 2;
            let new_setup_power = get_setup_size(proofs);
            assert_eq!(setup_power + 1, new_setup_power);
            setup_power = new_setup_power;
        }
    }
}
