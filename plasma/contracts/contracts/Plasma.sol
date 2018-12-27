pragma solidity ^0.4.24;

import "./Verifier.sol";
import "./VerificationKeys.sol";
import "./PlasmaStorage.sol";

// Single operator mode

contract PlasmaStub is VerificationKeys, PlasmaStorage {

    modifier active_only() {
        require(!stopped, "contract should not be globally stopped");
        _;
    }

    modifier operator_only() {
        require(operators[msg.sender] == true, "sender should be one of the operators");
        _;
    }

    // unit normalization functions
    function scaleIntoPlasmaUnitsFromWei(uint256 value)
    public
    pure
    returns (uint128) {
        uint256 den = DENOMINATOR;
        require(value % den == 0, "amount has higher precision than possible");
        uint256 scaled = value / den;
        require(scaled < uint256(1) << 128, "deposit amount is too high");
        return uint128(scaled);
    }


    function scaleFromPlasmaUnitsIntoWei(uint128 value)
    public
    pure
    returns (uint256) {
        return uint256(value) * DENOMINATOR;
    }

    // stubs
    // verification
    function verifyProof(Circuit, uint256[8] memory, bytes32, bytes32, bytes32) internal view returns (bool valid);
}

contract Plasma is PlasmaStub, Verifier {
    // Implementation

    function verifyProof(Circuit circuitType, uint256[8] memory proof, bytes32 oldRoot, bytes32 newRoot, bytes32 finalHash)
        internal view returns (bool valid)
    {
        uint256 mask = (~uint256(0)) >> 3;
        uint256[14] memory vk;
        uint256[] memory gammaABC;
        if (circuitType == Circuit.DEPOSIT) {
            (vk, gammaABC) = getVkDepositCircuit();
        } else if (circuitType == Circuit.TRANSFER) {
            (vk, gammaABC) = getVkTransferCircuit();
        } else if (circuitType == Circuit.EXIT) {
            (vk, gammaABC) = getVkExitCircuit();
        } else {
            return false;
        }
        uint256[] memory inputs = new uint256[](3);
        inputs[0] = uint256(oldRoot);
        inputs[1] = uint256(newRoot);
        inputs[2] = uint256(finalHash) & mask;
        return Verify(vk, gammaABC, proof, inputs);
    }

}