
pragma solidity >=0.5.0 <0.7.0;

import "./PlonkCore.sol";

// Hardcoded constants to avoid accessing store
contract KeysWithPlonkVerifier is VerifierWithDeserialize {

    function isBlockSizeSupportedInternal(uint32 _size) internal pure returns (bool) {
        {{~#each chunks ~}}
        {{#if @first}}
        if (_size == uint32({{this}})) { return true; }
        {{~else}}
        else if (_size == uint32({{this}})) { return true; }
        {{~/if}}
        {{~ /each}}
        else { return false; }
    }

    function getVkBlock(uint32 _chunks) internal pure returns (VerificationKey memory vk) {
        {{~#each chunks ~}}
        {{#if @first}}
        if (_chunks == uint32({{this}})) { return getVkBlock{{this}}(); }
        {{~else}}
        else if (_chunks == uint32({{this}})) { return getVkBlock{{this}}(); }
        {{~/if}}
        {{~ /each}}
    }

    {{#each keys}}
    function {{key_getter_name}}() internal pure returns(VerificationKey memory vk) {
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
            [{{g2_x_x_c1}},
             {{g2_x_x_c0}}],
            [{{g2_x_y_c1}},
             {{g2_x_y_c0}}]
        );
    }
    {{/each}}

}
