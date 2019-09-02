pragma solidity ^0.5.8;

import "./PriorityQueue.sol";
import "./Franklin.sol";

contract TestPriorityQueue {
    PriorityQueue priorityQueue;
    Franklin franklin;

    constructor(address franklinAddress, address priorityQueueAddress) public {
        franklin = Franklin(franklinAddress);
        priorityQueue = PriorityQueue(priorityQueueAddress);
    }

    function testAddDepositRequests(
        address sender,
        address toAddress,
        uint16 token,
        uint112 amount,
        bytes20 signature
    ) external view returns (bool) {
        uint beforeCount = priorityQueue.totalRequests;
        priorityQueue.addDepositRequest(sender, toAddress, token, amount, signature);
        uint afterCount = priorityQueue.totalRequests;
        return afterCount - beforeCount == 1;
    }

    function testAddExitRequests(
        address fromAddress,
        address ethereumAddress,
        uint16 token,
        bytes20 signature
    ) external view returns (bool) {
        uint beforeCount = priorityQueue.totalRequests;
        priorityQueue.addExitRequest(fromAddress, ethereumAddress, token, signature);
        uint afterCount = priorityQueue.totalRequests;
        return afterCount - beforeCount == 1;
    }

    function testAllRequestsRemoved() external view returns (bool) {
        return priorityQueue.totalRequests == 0;
    }

    function testIsExodusActivate() external view returns (bool) {
        return franklin.isExodusActivated(0);
    }
}