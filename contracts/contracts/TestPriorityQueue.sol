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

    // TODO
    function testAdd10DepositRequests() external view returns (bool) {
        uint beforeCount = priorityQueue.totalRequests;
        priorityQueue.addDepositRequest(sender, toAddress, token, amount, signature);
        uint afterCount = priorityQueue.totalRequests;
        return afterCount - beforeCount == 1;
    }

    // TODO
    function testAdd10ExitRequests() external view returns (bool) {
        uint beforeCount = priorityQueue.totalRequests;
        priorityQueue.addExitRequest(fromAddress, ethereumAddress, token, signature);
        uint afterCount = priorityQueue.totalRequests;
        return afterCount - beforeCount == 1;
    }

    // TODO
    function testAllRequestsRemoved() external view returns (bool) {
        return priorityQueue.totalRequests == 0;
    }

    // TODO
    function testIsExodusActivate() external view returns (bool) {
        return franklin.isExodusActivated(0);
    }
}