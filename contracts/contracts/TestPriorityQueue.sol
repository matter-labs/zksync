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

    function testAllRequestsRemoved() external view returns (bool) {
        return priorityQueue.totalRequests == 0;
    }

    function testIsExodusActivated() external view returns (bool) {
        return priorityQueue.isExodusActivated(block.number);
    }

    function testExodusAlwaysActivated() external view returns (bool) {
        return priorityQueue.isExodusActivated(0);
    }
}