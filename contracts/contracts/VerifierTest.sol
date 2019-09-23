// from https://github.com/HarryR/ethsnarks/blob/master/contracts/Verifier.sol
pragma solidity ^0.5.8;

import "./VerificationKey.sol";

contract VerifierTest is VerificationKey {

    // Proof verification
    // Params:
    // - _proof - block number
    // - _commitment - block commitment
    function verifyBlockProof(
        uint256[8] calldata _proof,
        bytes32 _commitment
    ) external view returns (bool) {
        uint256 mask = (~uint256(0)) >> 3;
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        (vk, gammaABC) = getVk();
        uint256[] memory inputs = new uint256[](1);
        inputs[0] = uint256(_commitment) & mask;
        return Verify(vk, gammaABC, _proof, inputs);
    }

    function verifyExitProof(
        uint16 _tokenId,
        address _owner,
        uint128 _amount,
        uint256[8] calldata _proof
    ) external view returns (bool) {
        bytes32 hash = sha256(
            abi.encodePacked(uint256(_tokenId), uint256(_owner))
        );
        hash = sha256(abi.encodePacked(hash, uint256(_amount)));

        uint256 mask = (~uint256(0)) >> 3;
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        (vk, gammaABC) = getVk();
        uint256[] memory inputs = new uint256[](1);
        inputs[0] = uint256(hash) & mask;
        return Verify(vk, gammaABC, _proof, inputs);
    }

    function NegateY(uint256 Y) internal pure returns (uint256) {
        uint256 q = 21888242871839275222246405745257275088696311157297823662689037894645226208583;
        return q - (Y % q);
    }

    function Verify(
        uint256[14] memory in_vk,
        uint256[] memory vk_gammaABC,
        uint256[8] memory in_proof,
        uint256[] memory proof_inputs
    ) internal view returns (bool) {
        return true;
    }
}