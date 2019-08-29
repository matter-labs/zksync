pragma solidity ^0.5.8;

import "./ExitQueue.sol";
import "./Franklin.sol";

contract TestExitQueue {
    ExitQueue exitQueue;
    Franklin franklin;

    bool internal enoughRequests;

    modifier enoughRequestsAdded() {
        require(enoughRequests, "Not the main Franklin contract");
        _;
    }

    constructor(address franklinAddress, address exitQueueAddress) public {
        franklin = Franklin(franklinAddress);
        exitQueue = ExitQueue(exitQueueAddress);
    }

    // TODO
    function testAdd10Requests() external view returns (bool) {
        exitQueue.addRequest(accountAddress, ethereumAddress, token, signature);
        exitQueue.addRequest(accountAddress, ethereumAddress, token, signature);
        exitQueue.addRequest(accountAddress, ethereumAddress, token, signature);
        exitQueue.addRequest(accountAddress, ethereumAddress, token, signature);
        exitQueue.addRequest(accountAddress, ethereumAddress, token, signature);
        exitQueue.addRequest(accountAddress, ethereumAddress, token, signature);
        exitQueue.addRequest(accountAddress, ethereumAddress, token, signature);
        exitQueue.addRequest(accountAddress, ethereumAddress, token, signature);
        exitQueue.addRequest(accountAddress, ethereumAddress, token, signature);
        exitQueue.addRequest(accountAddress, ethereumAddress, token, signature);
        if (exitQueue.totalRequests == 10) {
            enoughRequests = true;
            return true;
        }
        return false;
    }

    // TODO
    function testGet5Requests() external view enoughRequestsAdded returns (bool) {
        bytes memory requests = exitQueue.getRequests(5);
        return requests.length == 5 * exitQueue.requestLength;
    }

    // TODO
    function testAllRequestsRemoved() external view enoughRequestsAdded returns (bool) {
        return exitQueue.totalRequests == 0;
    }

    // TODO
    function testIsExodusActivate() external view enoughRequestsAdded returns (bool) {
        return franklin.isExodusActivated(0);
    }
}