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

    // function testAddRequest(bytes calldata pubData) external view returns (bool) {
    //     uint beforeCount = priorityQueue.totalRequests;
    //     franklin.deposit(pubData);
    //     uint afterCount = priorityQueue.totalRequests;
    //     return afterCount - beforeCount == 1;
    // }

    // function testAllRequestsRemoved() external view returns (bool) {
    //     return priorityQueue.totalRequests == 0;
    // }

    // function testIsExodusActivate() external view returns (bool) {
    //     return franklin.isExodusActivated(0);
    // }
}