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
        uint beforeCount = priorityQueue.totalDepositRequests;
        priorityQueue.addDepositRequest(fromAddress, ethereumAddress, token, signature);
        priorityQueue.addDepositRequest(fromAddress, ethereumAddress, token, signature);
        priorityQueue.addDepositRequest(fromAddress, ethereumAddress, token, signature);
        priorityQueue.addDepositRequest(fromAddress, ethereumAddress, token, signature);
        priorityQueue.addDepositRequest(fromAddress, ethereumAddress, token, signature);
        priorityQueue.addDepositRequest(fromAddress, ethereumAddress, token, signature);
        priorityQueue.addDepositRequest(fromAddress, ethereumAddress, token, signature);
        priorityQueue.addDepositRequest(fromAddress, ethereumAddress, token, signature);
        priorityQueue.addDepositRequest(fromAddress, ethereumAddress, token, signature);
        priorityQueue.addDepositRequest(fromAddress, ethereumAddress, token, signature);
        uint afterCount = priorityQueue.totalDepositRequests;
        return afterCount - beforeCount == 10;
    }

    // TODO
    function testGet5DepositRequests() external view returns (bool) {
        require(priorityQueue.totalDepositRequests >= 5, "Not enough deposit requests added");
        bytes memory requests = priorityQueue.getDepositRequests(5);
        return requests.length == 5 * priorityQueue.depositRequestLength;
    }

    // TODO
    function testAdd10ExitRequests() external view returns (bool) {
        uint beforeCount = priorityQueue.totalExitRequests;
        priorityQueue.addExitRequest(sender, toAddress, token, amount, signature);
        priorityQueue.addExitRequest(sender, toAddress, token, amount, signature);
        priorityQueue.addExitRequest(sender, toAddress, token, amount, signature);
        priorityQueue.addExitRequest(sender, toAddress, token, amount, signature);
        priorityQueue.addExitRequest(sender, toAddress, token, amount, signature);
        priorityQueue.addExitRequest(sender, toAddress, token, amount, signature);
        priorityQueue.addExitRequest(sender, toAddress, token, amount, signature);
        priorityQueue.addExitRequest(sender, toAddress, token, amount, signature);
        priorityQueue.addExitRequest(sender, toAddress, token, amount, signature);
        priorityQueue.addExitRequest(sender, toAddress, token, amount, signature);
        uint afterCount = priorityQueue.totalExitRequests;
        return afterCount - beforeCount == 10;
    }

    // TODO
    function testGet5ExitRequests() external view returns (bool) {
        require(priorityQueue.totalExitRequests >= 5, "Not enough exit requests added");
        bytes memory requests = priorityQueue.getExitRequests(5);
        return requests.length == 5 * priorityQueue.exitRequestLength;
    }

    // TODO
    function testAllDepositRequestsRemoved() external view returns (bool) {
        return priorityQueue.totalDepositRequests == 0;
    }

    // TODO
    function testAllExitRequestsRemoved() external view returns (bool) {
        return priorityQueue.totalExitRequests == 0;
    }

    // TODO
    function testIsExodusActivate() external view returns (bool) {
        return franklin.isExodusActivated(0);
    }
}