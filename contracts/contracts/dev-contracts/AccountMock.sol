// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.7.0;

import "./IEIP1271.sol";

/// Test representation of "smart wallet" which implements EIP-1271 interface.
contract AccountMock is IEIP1271 {
    address public owner;

    // bytes4(keccak256("isValidSignature(bytes32,bytes)")
    bytes4 internal constant EIP1271_SUCCESS_RETURN_VALUE = 0x1626ba7e;

    constructor(address _owner) {
        owner = _owner;
    }

    function isValidSignature(bytes32 _hash, bytes memory _signature) public view override returns (bytes4) {
        require(_signature.length == 65, "sign.len incorrect");
        uint8 v;
        bytes32 r;
        bytes32 s;
        // Signature loading code (together with the comment is taken from the Argent smart contract).
        // we jump 32 (0x20) as the first slot of bytes contains the length
        // we jump 65 (0x41) per signature
        // for v we load 32 bytes ending with v (the first 31 come from s) then apply a mask
        // solhint-disable-next-line no-inline-assembly
        assembly {
            r := mload(add(_signature, 0x20))
            s := mload(add(_signature, 0x40))
            v := and(mload(add(_signature, 0x41)), 0xff)
        }
        require(v == 27 || v == 28, "sign.v != 27 || 28");

        address recoveredAddress = ecrecover(_hash, v, r, s);
        require(recoveredAddress != address(0), "ecrecover returned 0");
        require(recoveredAddress == owner, "rec. addr != owner");

        return EIP1271_SUCCESS_RETURN_VALUE;
    }
}
