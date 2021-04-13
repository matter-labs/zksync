// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

// solhint-disable-next-line compiler-version
pragma abicoder v2;

import "../Operations.sol";

contract OperationsTest {
    function testDepositPubdata(Operations.Deposit memory _example, bytes memory _pubdata) external pure {
        Operations.Deposit memory parsed = Operations.readDepositPubdata(_pubdata);
        require(_example.tokenId == parsed.tokenId, "tok");
        require(_example.amount == parsed.amount, "amn");
        require(_example.owner == parsed.owner, "own");
    }

    function testDepositPriorityQueue(Operations.Deposit memory _example, bytes memory _priorityQueueData)
        external
        pure
    {
        bytes memory result = Operations.writeDepositPubdataForPriorityQueue(_example);
        require(keccak256(result) == keccak256(_priorityQueueData), "pqd");
    }

    function testFullExitPubdata(Operations.FullExit memory _example, bytes memory _pubdata) external pure {
        Operations.FullExit memory parsed = Operations.readFullExitPubdata(_pubdata);
        require(_example.accountId == parsed.accountId, "acc");
        require(_example.owner == parsed.owner, "own");
        require(_example.tokenId == parsed.tokenId, "tok");
        require(_example.amount == parsed.amount, "amn");
    }

    function testFullExitPriorityQueue(Operations.FullExit memory _example, bytes memory _priorityQueueData)
        external
        pure
    {
        bytes memory result = Operations.writeFullExitPubdataForPriorityQueue(_example);
        require(keccak256(result) == keccak256(_priorityQueueData), "pqd");
    }

    function testWithdrawPubdata(Operations.PartialExit memory _example, bytes memory _pubdata) external pure {
        Operations.PartialExit memory parsed = Operations.readPartialExitPubdata(_pubdata);
        require(_example.owner == parsed.owner, "own");
        require(_example.tokenId == parsed.tokenId, "tok");
        require(_example.amount == parsed.amount, "amn");
    }

    function testForcedExitPubdata(Operations.ForcedExit memory _example, bytes memory _pubdata) external pure {
        Operations.ForcedExit memory parsed = Operations.readForcedExitPubdata(_pubdata);
        require(_example.target == parsed.target, "trg");
        require(_example.tokenId == parsed.tokenId, "tok");
        require(_example.amount == parsed.amount, "amn");
    }

    function testChangePubkeyPubdata(Operations.ChangePubKey memory _example, bytes memory _pubdata) external pure {
        Operations.ChangePubKey memory parsed = Operations.readChangePubKeyPubdata(_pubdata);
        require(_example.accountId == parsed.accountId, "acc");
        require(_example.pubKeyHash == parsed.pubKeyHash, "pkh");
        require(_example.owner == parsed.owner, "own");
        require(_example.nonce == parsed.nonce, "nnc");
    }
}
