pragma solidity 0.5.10;

import "./Verifier.sol";
import "./VerificationKey.sol";
import "./Franklin.sol";

contract VerifyTest {
    Verifier verifier;

    constructor(address _verifierAddress) public {
        verifier = Verifier(_verifierAddress);
    }

    function verifyProof(bytes32 commitment, uint256[8] calldata proof)
        external
        view
    {
        require(verifier.verifyBlockProof(proof, commitment), "verification failed");
    }
}
