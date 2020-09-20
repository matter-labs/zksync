// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

import "generated/ZkSyncTest.sol";


contract ZkSyncProcessOpUnitTest is ZkSyncTest {

    function testProcessOperation(
        bytes calldata _publicData,
        bytes calldata _ethWitness,
        uint32[] calldata _ethWitnessSizes
    ) external {
        collectOnchainOps(0, _publicData, _ethWitness, _ethWitnessSizes);
    }

}
