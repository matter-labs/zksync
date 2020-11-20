pragma solidity ^0.5.8;

import "IEIP1271.sol";

contract EIP1271 is IEIP1271 {
    // bytes4(keccak256("isValidSignature(bytes32,bytes)")
    bytes4 internal constant EIP1271_SUCCESS_RETURN_VALUE = 0x1626ba7e;

    /**
     * @dev Should return whether the signature provided is valid for the provided data
     * @param _hash Hash which was signed on the behalf of address(this)
     * @param _signature Signature byte array associated with _data
     *
     * MUST return the bytes4 magic value 0x1626ba7e when function passes.
     * MUST NOT modify state (using STATICCALL for solc < 0.5, view modifier for solc > 0.5)
     * MUST allow external calls
     */
    function isValidSignature(bytes32 _hash, bytes memory _signature) public view returns (bytes4) {
        return EIP1271_SUCCESS_RETURN_VALUE;
    }
}
