pragma solidity ^0.5.8;

import "./KeysWithPlonkVerifier.sol";

// Hardcoded constants to avoid accessing store
contract Verifier is KeysWithPlonkVerifier {
    bool constant DUMMY_VERIFIER = false;

    function initialize(bytes calldata) external {}

    /// @notice Verifier contract upgrade. Can be external because Proxy contract intercepts illegal calls of this function.
    /// @param upgradeParameters Encoded representation of upgrade parameters
    function upgrade(bytes calldata upgradeParameters) external {}

    function isBlockSizeSupported(uint32 _size) public pure returns (bool) {
        if (DUMMY_VERIFIER) {
            return true;
        } else {
            return isBlockSizeSupportedInternal(_size);
        }
    }

    function verifyBlockProof(
        uint[] calldata _proof,
        bytes32 _commitment,
        uint32 _chunks
    ) external view returns (bool) {
        if (DUMMY_VERIFIER) {
            uint oldGasValue = gasleft();
            uint tmp;
            while (gasleft() + 470000 > oldGasValue) {
                tmp += 1;
            }
            return true;
        }
        uint[] memory inputs = new uint[](1);
        uint mask = (~uint(0)) >> 3;
        inputs[0] = uint(_commitment) & mask;
        Proof memory proof = deserialize_proof(inputs, _proof);
        VerificationKey memory vk = getVkBlock(_chunks);
        require(vk.num_inputs == inputs.length);
        return verify(proof, vk);
    }

    function verifyExitProof(
        bytes32 _rootHash,
        uint32 _accountId,
        address _owner,
        uint16 _tokenId,
        uint128 _amount,
        uint[] calldata _proof
    ) external view returns (bool) {
        bytes32 commitment = sha256(abi.encodePacked(_rootHash, _accountId, _owner, _tokenId, _amount));

        uint[] memory inputs = new uint[](1);
        uint mask = (~uint(0)) >> 3;
        inputs[0] = uint(commitment) & mask;
        Proof memory proof = deserialize_proof(inputs, _proof);
        VerificationKey memory vk = getVkExit();
        require(vk.num_inputs == inputs.length);
        return verify(proof, vk);
    }
}
