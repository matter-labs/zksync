// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "../ZkSync.sol";
import "../AdditionalZkSync.sol";

contract ZkSyncRegenesisTest is ZkSync {
    function getStoredBlockHash() external view returns (bytes32) {
        require(totalBlocksCommitted == totalBlocksProven, "wq1"); // All the blocks must be processed
        require(totalBlocksCommitted == totalBlocksExecuted, "w12"); // All the blocks must be processed

        return storedBlockHashes[totalBlocksExecuted];
    }

    function getAdditionalZkSync() external view returns (AdditionalZkSync) {
        return additionalZkSync;
    }
}
