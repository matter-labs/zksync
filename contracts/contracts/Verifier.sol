pragma solidity >=0.5.0 <0.7.0;

import "./KeysWithPlonkVerifier.sol";

// Hardcoded constants to avoid accessing store
contract Verifier is KeysWithPlonkVerifier {

    bool constant DUMMY_VERIFIER = false;

    constructor() public {}
    function initialize(bytes calldata) external {
    }


    function isBlockSizeSupported(uint32 _size) public pure returns (bool) {
        if (DUMMY_VERIFIER) {
            return true;
        } else {
            return isBlockSizeSupportedInternal(_size);
        }
    }

    function verifyBlockProof(
        uint256[] calldata _proof,
        bytes32 _commitment,
        uint32 _chunks
    ) external view returns (bool) {
        if (DUMMY_VERIFIER) {
            uint oldGasValue = gasleft();
            uint tmp;
            while (gasleft() > oldGasValue - 470000) {
                tmp += 1;
            }
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
