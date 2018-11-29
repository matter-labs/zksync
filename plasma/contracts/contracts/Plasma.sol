pragma solidity ^0.5.0;

import "./Verifier.sol";
import "./VerificationKeys.sol";


contract Plasma is Verifier, VerificationKeys {

    // Public API

    constructor() public {}

//    // Deposit ERC20 tokens
//    function deposit(address /*_from*/, uint /*_amount*/) public {
//
//    }

    function commitBlock(uint256 blockNumber, uint256 totalFees, bytes memory txDataPacked, bytes32 newRoot) public {
        bytes32 initialHash = sha256(abi.encodePacked(blockNumber, totalFees));
        bytes32 finalHash = sha256(abi.encodePacked(initialHash, txDataPacked));

    }

    function verifyBlock() public {

    }

    // Implementation

    function verifyUpdateProof(uint256[8] memory proof, bytes32 oldRoot, bytes32 newRoot, bytes32 finalHash)
        internal view returns (bool valid)
    {
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        (vk, gammaABC) = getVkUpdateCircuit();
        uint256[] memory inputs = new uint256[](3);
        inputs[0] = uint256(oldRoot);
        inputs[1] = uint256(newRoot);
        inputs[2] = uint256(finalHash);
        return Verify(vk, gammaABC, proof, inputs);
    }



}
