pragma solidity ^0.7.0;
pragma experimental ABIEncoderV2;

// SPDX-License-Identifier: UNLICENSED





import "../ZkSync.sol";

contract ZkSyncProcessOpUnitTest is ZkSync {
    function collectOnchainOpsExternal(CommitBlockInfo memory _newBlockData, bytes32 processableOperationsHash, uint64 priorityOperationsProcessed, bytes memory offsetsCommitment)
    external
    {
        (bytes32 resOpHash, uint64 resPriorOps, bytes memory resOffsetsCommitment) = collectOnchainOps(_newBlockData);
        require(resOpHash == processableOperationsHash, "hash");
        require(resPriorOps == priorityOperationsProcessed, "prop");
        require(keccak256(resOffsetsCommitment) == keccak256(offsetsCommitment), "offComm");
    }

    function commitPriorityRequests() external {
        totalCommittedPriorityRequests = totalOpenPriorityRequests;
    }
}
