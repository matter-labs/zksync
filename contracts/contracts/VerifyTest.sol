pragma solidity ^0.5.1;

import "./Verifier.sol";
import "./VerificationKey.sol";
import "./Franklin.sol";

contract VerifyTest {
    VerificationKey verificationKey;
    Verifier verifier;

    constructor(address _verifierAddress, address _vkAddress) public {
        verifier = Verifier(_verifierAddress);
        verificationKey = VerificationKey(_vkAddress);
    }

    function verifyProof(bytes32 commitment, uint256[8] calldata proof)
        external
    {
        require(verifyBlockProof(proof, commitment), "verification failed");
    }

    function verifyBlockProof(uint256[8] memory proof, bytes32 commitment)
        internal
        view
        returns (bool valid)
    {
        uint256 mask = (~uint256(0)) >> 3;
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        (vk, gammaABC) = verificationKey.getVk();
        uint256[] memory inputs = new uint256[](1);
        inputs[0] = uint256(commitment) & mask;
        return verifier.Verify(vk, gammaABC, proof, inputs);
    }

}
