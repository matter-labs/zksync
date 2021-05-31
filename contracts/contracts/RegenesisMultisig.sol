// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

import "./Ownable.sol";
import "./Utils.sol";

pragma experimental ABIEncoderV2;

/// @title Regenesis Multisig contract
/// @author Matter Labs
contract RegenesisMultisig is Ownable {
    address public gnosisAddress;

    bytes32 public oldRootHash;
    bytes32 public newRootHash;

    constructor(address _gnosisAddress) Ownable(msg.sender) {
        gnosisAddress = _gnosisAddress;
    }

    function submitHash(
        bytes32 _oldRootHash,
        bytes32 _newRootHash
    ) external {
        // Only gnosis multisig of the security council can submit 
        // the new root hash
        require(msg.sender == gnosisAddress, "1"); 
        
        oldRootHash = _oldRootHash;
        newRootHash = _newRootHash;
    }
}
