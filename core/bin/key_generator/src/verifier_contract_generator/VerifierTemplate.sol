// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "./PlonkCore.sol";

// Hardcoded constants to avoid accessing store
contract KeysWithPlonkVerifier is VerifierWithDeserialize {

    uint256 constant VK_TREE_ROOT = {{vk_tree_root}};
    uint8 constant VK_MAX_INDEX = {{vk_max_index}};

    function getVkAggregated(uint32 _proofs) internal pure returns (VerificationKey memory vk) {
        {{~#each sizes ~}}
        {{#if @first}}
        if (_proofs == uint32({{this}})) { return getVkAggregated{{this}}(); }
        {{~else}}
        else if (_proofs == uint32({{this}})) { return getVkAggregated{{this}}(); }
        {{~/if}}
        {{~ /each}}
    }

    {{#each keys}}
    function {{key_getter_name}}() internal pure returns(VerificationKey memory vk) {
        vk.domain_size = {{domain_size}};
        vk.num_inputs = {{num_inputs}};
        vk.omega = PairingsBn254.new_fr({{omega}});
        vk.gate_setup_commitments[0] = PairingsBn254.new_g1(
            {{gate_setup_commitment_0_0}},
            {{gate_setup_commitment_0_1}}
        );
        vk.gate_setup_commitments[1] = PairingsBn254.new_g1(
            {{gate_setup_commitment_1_0}},
            {{gate_setup_commitment_1_1}}
        );
        vk.gate_setup_commitments[2] = PairingsBn254.new_g1(
            {{gate_setup_commitment_2_0}},
            {{gate_setup_commitment_2_1}}
        );
        vk.gate_setup_commitments[3] = PairingsBn254.new_g1(
            {{gate_setup_commitment_3_0}},
            {{gate_setup_commitment_3_1}}
        );
        vk.gate_setup_commitments[4] = PairingsBn254.new_g1(
            {{gate_setup_commitment_4_0}},
            {{gate_setup_commitment_4_1}}
        );
        vk.gate_setup_commitments[5] = PairingsBn254.new_g1(
            {{gate_setup_commitment_5_0}},
            {{gate_setup_commitment_5_1}}
        );
        vk.gate_setup_commitments[6] = PairingsBn254.new_g1(
            {{gate_setup_commitment_6_0}},
            {{gate_setup_commitment_6_1}}
        );
        vk.gate_selector_commitments[0] = PairingsBn254.new_g1(
            {{gate_selector_commitment_0_0}},
            {{gate_selector_commitment_0_1}}
        );
        vk.gate_selector_commitments[1] = PairingsBn254.new_g1(
            {{gate_selector_commitment_1_0}},
            {{gate_selector_commitment_1_1}}
        );
        vk.copy_permutation_commitments[0] = PairingsBn254.new_g1(
            {{permutation_commitment_0_0}},
            {{permutation_commitment_0_1}}
        );
        vk.copy_permutation_commitments[1] = PairingsBn254.new_g1(
            {{permutation_commitment_1_0}},
            {{permutation_commitment_1_1}}
        );
        vk.copy_permutation_commitments[2] = PairingsBn254.new_g1(
            {{permutation_commitment_2_0}},
            {{permutation_commitment_2_1}}
        );
        vk.copy_permutation_commitments[3] = PairingsBn254.new_g1(
            {{permutation_commitment_3_0}},
            {{permutation_commitment_3_1}}
        );
        vk.copy_permutation_non_residues[0] = PairingsBn254.new_fr(
            {{permutation_non_residue_0}}
        );
        vk.copy_permutation_non_residues[1] = PairingsBn254.new_fr(
            {{permutation_non_residue_1}}
        );
        vk.copy_permutation_non_residues[2] = PairingsBn254.new_fr(
            {{permutation_non_residue_2}}
        );

        vk.g2_x = PairingsBn254.new_g2(
            [{{g2_x_x_c1}},
            {{g2_x_x_c0}}],
            [{{g2_x_y_c1}},
            {{g2_x_y_c0}}]
        );
    }
    {{/each}}

}

// Hardcoded constants to avoid accessing store
contract KeysWithPlonkVerifierOld is VerifierWithDeserializeOld {

    {{#each single_keys}}
    function {{key_getter_name}}() internal pure returns(VerificationKeyOld memory vk) {
        vk.domain_size = {{domain_size}};
        vk.num_inputs = {{num_inputs}};
        vk.omega = PairingsBn254.new_fr({{omega}});
        vk.selector_commitments[0] = PairingsBn254.new_g1(
            {{selector_commitment_0_0}},
            {{selector_commitment_0_1}}
        );
        vk.selector_commitments[1] = PairingsBn254.new_g1(
            {{selector_commitment_1_0}},
            {{selector_commitment_1_1}}
        );
        vk.selector_commitments[2] = PairingsBn254.new_g1(
            {{selector_commitment_2_0}},
            {{selector_commitment_2_1}}
        );
        vk.selector_commitments[3] = PairingsBn254.new_g1(
            {{selector_commitment_3_0}},
            {{selector_commitment_3_1}}
        );
        vk.selector_commitments[4] = PairingsBn254.new_g1(
            {{selector_commitment_4_0}},
            {{selector_commitment_4_1}}
        );
        vk.selector_commitments[5] = PairingsBn254.new_g1(
            {{selector_commitment_5_0}},
            {{selector_commitment_5_1}}
        );

        // we only have access to value of the d(x) witness polynomial on the next
        // trace step, so we only need one element here and deal with it in other places
        // by having this in mind
        vk.next_step_selector_commitments[0] = PairingsBn254.new_g1(
            {{next_step_selector_commitment_0_0}},
            {{next_step_selector_commitment_0_1}}
        );

        vk.permutation_commitments[0] = PairingsBn254.new_g1(
            {{permutation_commitment_0_0}},
            {{permutation_commitment_0_1}}
        );
        vk.permutation_commitments[1] = PairingsBn254.new_g1(
            {{permutation_commitment_1_0}},
            {{permutation_commitment_1_1}}
        );
        vk.permutation_commitments[2] = PairingsBn254.new_g1(
            {{permutation_commitment_2_0}},
            {{permutation_commitment_2_1}}
        );
        vk.permutation_commitments[3] = PairingsBn254.new_g1(
            {{permutation_commitment_3_0}},
            {{permutation_commitment_3_1}}
        );

        vk.permutation_non_residues[0] = PairingsBn254.new_fr(
            {{permutation_non_residue_0}}
        );
        vk.permutation_non_residues[1] = PairingsBn254.new_fr(
            {{permutation_non_residue_1}}
        );
        vk.permutation_non_residues[2] = PairingsBn254.new_fr(
            {{permutation_non_residue_2}}
        );

        vk.g2_x = PairingsBn254.new_g2(
            [{{g2_x_x_c1}}, {{g2_x_x_c0}}],
            [{{g2_x_y_c1}}, {{g2_x_y_c0}}]
        );
    }
    {{/each}}
}
