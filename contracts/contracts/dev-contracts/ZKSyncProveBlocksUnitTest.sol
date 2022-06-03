// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "../ZkSync.sol";

contract ZKSyncProveBlocksUnitTest is ZkSync {
    function initializeNumberOfCommittedAndProvedBlocks(uint32 _numberOfCommittedBlocks, uint32 _numberOfProvedBlocks)
        external
    {
        for (uint32 i = 0; i <= _numberOfCommittedBlocks; ++i) {
            StoredBlockInfo memory storedBlockTemp = StoredBlockInfo(i, 0, 0, 0, 0, 0);
            storedBlockHashes[i] = hashStoredBlockInfo(storedBlockTemp);
        }
        totalBlocksCommitted = _numberOfCommittedBlocks;
        totalBlocksProven = _numberOfProvedBlocks;
    }

    function proveBlocksTest(uint32 _startBlockToBeProved, uint32 _endBlockToBeProved) external {
        StoredBlockInfo[] memory _committedBlocks = new StoredBlockInfo[](
            _endBlockToBeProved - _startBlockToBeProved + 1
        );
        uint256[16] memory subproofsLimbsTemp;
        ProofInput memory proofInput = ProofInput(
            new uint256[](0),
            new uint256[](0),
            new uint256[](_committedBlocks.length),
            new uint8[](0),
            subproofsLimbsTemp
        );

        for (uint32 i = 0; i < _committedBlocks.length; ++i) {
            StoredBlockInfo memory storedBlockTemp = StoredBlockInfo(i + _startBlockToBeProved, 0, 0, 0, 0, 0);
            _committedBlocks[i] = storedBlockTemp;
        }
        this.proveBlocks(_committedBlocks, proofInput);
    }
}
