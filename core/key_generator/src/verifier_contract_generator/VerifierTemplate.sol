pragma solidity >=0.5.0 <0.7.0;

import "./PlonkCore.sol";

// Hardcoded constants to avoid accessing store
contract Verifier is VerifierWithDeserialize{

    bool constant DUMMY_VERIFIER = false;

    constructor() public {}
    function initialize(bytes calldata) external {
    }

    function isBlockSizeSupported(uint32 _size) public pure returns (bool) {
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


    function verifyBlockProof(
        uint256[] calldata _proof,
        bytes32 _commitment,
        uint32 _chunks
    ) external view returns (bool) {
        if (DUMMY_VERIFIER) {
            return true;
        }
        uint256[] memory inputs = new uint256[](1);
        uint256 mask = (~uint256(0)) >> 3;
        inputs[0] = uint256(_commitment) & mask;
        Proof memory proof = deserialize_proof(1, inputs, _proof);
        VerificationKey memory vk = getVkBlock(_chunks);
        return verify(proof, vk);
    }

    function verifyExitProof(
        bytes32 _root_hash,
        address _owner,
        uint16 _tokenId,
        uint128 _amount,
        uint256[] calldata _proof
    ) external view returns (bool) {
        bytes32 commitment = sha256(abi.encodePacked(_root_hash, _owner, _tokenId, _amount));

        uint256[] memory inputs = new uint256[](1);
        uint256 mask = (~uint256(0)) >> 3;
        inputs[0] = uint256(commitment) & mask;
        Proof memory proof = deserialize_proof(1, inputs, _proof);
        VerificationKey memory vk = getVkExit();
        return verify(proof, vk);
    }
}
