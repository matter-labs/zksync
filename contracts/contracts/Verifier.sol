// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;
pragma experimental ABIEncoderV2;

import "./KeysWithPlonkVerifier.sol";

// Hardcoded constants to avoid accessing store
contract Verifier is KeysWithPlonkVerifier {
    bool constant DUMMY_VERIFIER = false;

    function initialize(bytes calldata) external {}

    /// @notice Verifier contract upgrade. Can be external because Proxy contract intercepts illegal calls of this function.
    /// @param upgradeParameters Encoded representation of upgrade parameters
    function upgrade(bytes calldata upgradeParameters) external {}

    function verifyAggregatedProof(
        uint256[] memory _recursiveInput,
        uint256[] memory _proof,
        uint8[] memory _vkIndexes,
        uint256[] memory _individual_vks_inputs,
        uint256[16] memory _subproofs_limbs
    ) external view returns (bool) {
        if (DUMMY_VERIFIER) {
            uint256 oldGasValue = gasleft();
            uint256 tmp;
            while (gasleft() + 500000 > oldGasValue) {
                tmp += 1;
            }
            return true;
        }
        for (uint256 i = 0; i < _individual_vks_inputs.length; ++i) {
            uint256 commitment = _individual_vks_inputs[i];
            uint256 mask = (~uint256(0)) >> 3;
            _individual_vks_inputs[i] = uint256(commitment) & mask;
        }
        VerificationKey memory vk = getVkAggregated(uint32(_vkIndexes.length));
        return
            verify_serialized_proof_with_recursion(
                _recursiveInput,
                _proof,
                VK_TREE_ROOT,
                VK_MAX_INDEX,
                _vkIndexes,
                _individual_vks_inputs,
                _subproofs_limbs,
                vk
            );
    }

    function verifyExitProof(
        bytes32 _rootHash,
        uint32 _accountId,
        address _owner,
        uint16 _tokenId,
        uint128 _amount,
        uint256[] calldata _proof
    ) external view returns (bool) {
        bytes32 commitment = sha256(abi.encodePacked(_rootHash, _accountId, _owner, _tokenId, _amount));

        uint256[] memory inputs = new uint256[](1);
        uint256 mask = (~uint256(0)) >> 3;
        inputs[0] = uint256(commitment) & mask;
        Proof memory proof = deserialize_proof(inputs, _proof);
        VerificationKey memory vk = getVkExit();
        require(vk.num_inputs == inputs.length);
        return verify(proof, vk);
    }
}
