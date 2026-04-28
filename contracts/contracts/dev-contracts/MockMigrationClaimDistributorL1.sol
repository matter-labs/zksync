// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.7.0;

contract MockMigrationClaimDistributorL1 {
    bytes32 public MERKLE_ROOT;

    constructor(bytes32 merkleRoot) {
        MERKLE_ROOT = merkleRoot;
    }

    receive() external payable {}
}
