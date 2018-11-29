// This contract is generated programmatically

pragma solidity ^0.4.24;


// Hardcoded constants to avoid accessing store
contract VerificationKeys {

    // For tree depth 24
    bytes32 constant EMPTY_TREE_ROOT = 0x1d3843a9bbf376e57b3eca393198d7211882f6f2a76a53730243e2a1a519d92a;

    function getVkUpdateCircuit() internal pure returns (uint256[14] memory vk, uint256[] memory gammaABC) {
        vk[0] = 0;
        gammaABC = new uint256[](1);
    }

}
