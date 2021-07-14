// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;
pragma experimental ABIEncoderV2;

import "./KeysWithPlonkVerifier.sol";
import "./Config.sol";

// Hardcoded constants to avoid accessing store
contract Verifier is KeysWithPlonkVerifier, KeysWithPlonkVerifierOld, Config {
    // solhint-disable-next-line no-empty-blocks
    function initialize(bytes calldata) external {}

    /// @notice Verifier contract upgrade. Can be external because Proxy contract intercepts illegal calls of this function.
    /// @param upgradeParameters Encoded representation of upgrade parameters
    // solhint-disable-next-line no-empty-blocks
    function upgrade(bytes calldata upgradeParameters) external {}

    function verifyAggregatedBlockProof(
        uint256[] memory _recursiveInput,
        uint256[] memory _proof,
        uint8[] memory _vkIndexes,
        uint256[] memory _individualVksInputs,
        uint256[16] memory _subproofsLimbs
    ) external view returns (bool) {
        // #if DUMMY_VERIFIER
        uint256 oldGasValue = gasleft();
        // HACK: ignore warnings from unused variables
        abi.encode(_recursiveInput, _proof, _vkIndexes, _individualVksInputs, _subproofsLimbs);
        uint256 tmp;
        while (gasleft() + 500000 > oldGasValue) {
            tmp += 1;
        }
        return true;
        // #else
        for (uint256 i = 0; i < _individualVksInputs.length; ++i) {
            uint256 commitment = _individualVksInputs[i];
            _individualVksInputs[i] = commitment & INPUT_MASK;
        }
        VerificationKey memory vk = getVkAggregated(uint32(_vkIndexes.length));

        return
            verify_serialized_proof_with_recursion(
                _recursiveInput,
                _proof,
                VK_TREE_ROOT,
                VK_MAX_INDEX,
                _vkIndexes,
                _individualVksInputs,
                _subproofsLimbs,
                vk
            );
        // #endif
    }

    function verifyExitProof(
        bytes32 _rootHash,
        uint32 _accountId,
        address _owner,
        uint32 _tokenId,
        uint128 _amount,
        uint32 _nftCreatorAccountId,
        address _nftCreatorAddress,
        uint32 _nftSerialId,
        bytes32 _nftContentHash,
        uint256[] calldata _proof
    ) external view returns (bool) {
        bytes32 commitment =
            sha256(
                abi.encodePacked(
                    _rootHash,
                    _accountId,
                    _owner,
                    _tokenId,
                    _amount,
                    _nftCreatorAccountId,
                    _nftCreatorAddress,
                    _nftSerialId,
                    _nftContentHash
                )
            );

        uint256[] memory inputs = new uint256[](1);
        inputs[0] = uint256(commitment) & INPUT_MASK;
        ProofOld memory proof = deserialize_proof_old(inputs, _proof);
        VerificationKeyOld memory vk = getVkExit();
        require(vk.num_inputs == inputs.length, "n1");
        return verify_old(proof, vk);
    }
}
