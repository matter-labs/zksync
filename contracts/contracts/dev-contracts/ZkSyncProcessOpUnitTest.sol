// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "../ZkSync.sol";


contract ZkSyncProcessOpUnitTest is ZkSync {

    function testProcessOperation(
        bytes calldata _publicData,
        bytes calldata _ethWitness,
        uint32[] calldata _ethWitnessSizes
    ) external {
        // todo: unimplemeneted
//        collectOnchainOps(0, _publicData, _ethWitness, _ethWitnessSizes);
    }

}
