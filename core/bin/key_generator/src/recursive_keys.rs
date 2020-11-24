use std::fs::File;
use zksync_crypto::params::{
    RECURSIVE_CIRCUIT_NUM_INPUTS, RECURSIVE_CIRCUIT_SIZES, RECURSIVE_CIRCUIT_VK_TREE_DEPTH,
};
use zksync_crypto::recursive_aggregation_circuit;
use zksync_prover_utils::fs_utils::{
    get_recursive_verification_key_path, get_universal_setup_monomial_form,
};

pub fn make_recursive_verification_keys() {
    for (proofs, setup_power) in RECURSIVE_CIRCUIT_SIZES {
        let path = get_recursive_verification_key_path(*proofs);
        log::info!(
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
                *proofs,
                RECURSIVE_CIRCUIT_NUM_INPUTS,
                RECURSIVE_CIRCUIT_VK_TREE_DEPTH,
                &get_universal_setup_monomial_form(*setup_power).expect("Universal setup no found"),
            )
            .expect("Failed to generate recursive circuit verification keys");
        vk.write(vk_file).expect("Failed to save verification key");
    }
}
