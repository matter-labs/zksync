// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "../ZkSync.sol";

contract ZkSyncProcessOpUnitTest is ZkSync {
    function collectOnchainOpsExternal(
        CommitBlockInfo memory _newBlockData,
        bytes32 processableOperationsHash,
        uint64 priorityOperationsProcessed,
        bytes memory offsetsCommitment
    ) external view {
        (bytes32 resOpHash, uint64 resPriorOps, bytes memory resOffsetsCommitment) = collectOnchainOps(_newBlockData);
        require(resOpHash == processableOperationsHash, "h");
        require(resPriorOps == priorityOperationsProcessed, "p");
        require(keccak256(resOffsetsCommitment) == keccak256(offsetsCommitment), "o");
    }

    function commitPriorityRequests() external {
        totalCommittedPriorityRequests = totalOpenPriorityRequests;
    }

    function getTotalOpenPriorityRequests() external view returns (uint64) {
        return totalOpenPriorityRequests;
    }

    function getTotalCommittedPriorityRequests() external view returns (uint64) {
        return totalCommittedPriorityRequests;
    }

    function getAuthFact(address _address, uint32 nonce) external view returns (bytes32) {
        return authFacts[_address][nonce];
    }
}
