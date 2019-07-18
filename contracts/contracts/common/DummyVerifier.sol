pragma solidity ^0.5.1;

contract DummyVerifier {

    function Verify (uint256[14] memory in_vk, uint256[] memory vk_gammaABC, uint256[8] memory in_proof, uint256[] memory proof_inputs)
    internal
    view
    returns (bool)
    {
        return true;
    }
}