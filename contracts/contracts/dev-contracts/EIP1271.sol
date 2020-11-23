// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

import "./IEIP1271.sol";

contract EIP1271 is IEIP1271 {
    // bytes4(keccak256("isValidSignature(bytes,bytes)")
    bytes4 internal constant EIP1271_SUCCESS_RETURN_VALUE = 0x20c13b0b;

    function isValidSignature(bytes32, bytes memory) public pure override returns (bytes4) {
        return EIP1271_SUCCESS_RETURN_VALUE;
    }
}
