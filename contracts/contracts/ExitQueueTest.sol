pragma solidity ^0.5.8;

import "./ExitQueue.sol";
import "./Franklin.sol";

contract ExitQueueTest {
    ExitQueue exitQueue;
    Franklin franklin;

    constructor() public {
        franklin = Franklin();
        exitQueue = ExitQueue(franklin);
    }

    // TODO
    function testAddRequests() external view returns (bool) {
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
        return exitQueue.totalRequests == 10;
    }

    // TODO
    function testGetRequests() external view returns (bool) {
        bytesm memory requests = exitQueue.getRequests(5);
        return requests.length == ;
    }

    // TODO
    function testRemoveRequests() external view returns (bool) {
        franklin.removeExitRequests(4);
        return exitQueue.totalRequests == 6;
    }

    // TODO
    function testIsExodusActivate() external view returns (bool) {
        return franklin.isExodusActivated(0);
    }
}