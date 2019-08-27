pragma solidity ^0.5.8;

import "openzeppelin-solidity/contracts/ownership/Ownable.sol";

contract ExitQueue is Ownable {
    // Contains not satisfied exit requests
    mapping(uint32 => address) private accountsQueue;
    mapping(address => bytes) private exitRequests; // TODO: -in bytes or struct?
    uint32 public totalRequests;

    // ExitRequest sctructure
    // TODO: - is it final?
    struct ExitRequest {
        address accountId;
        address ethereumAddress;
        uint32 blockNumber;
        uint16 tokenId;
        uint128 balance;
        uint128 fee;
        bytes32 signature;
        uint256[8] proof;
    }

    function addRequest(bytes memory publicData) external {
        // TODO: what chanks? - discuss
        // Pubdata:
        // ethereumAddress: 20
        // blockNumber: 32
        // tokenId: 2
        // fee: 1
        // signature: 32,
        // proof: 32
        
        require(exitRequests[msg.sender] == 0, "Exit request from this sender exists");
        accountsQueue[totalRequests] = msg.sender;
        exitRequests[msg.sender] = publicData;
        totalRequests++;
        // TODO: - who need to validate proof?
        // TODO: - need to unpack?
    }

    function getRequests(uint32 requestsCount) external returns (ExitRequest[] memory) {
        require(totalRequests > 0, "No exit requests");
        uint32 count = requestsCount;
        if (totalRequests < requestsCount) {
            count = totalRequests;
        }
        ExitRequest[count] requests = new ExitRequest(count);
        for (uint32 i = 0; i < count; i++) {
            requests[i] = exitRequests[accountsQueue[i]].toExitRequest();
        }
        return requests;
    }

    function toExitRequest(bytes memory publicData) internal returns (ExitRequest) {
        return ExitRequest({
            /// TODO
        });
    }

    function removeRequests(uint32 requestsCount) external {
        require(totalRequests > 0, "No exit requests");
        uint32 count = requestsCount;
        if (totalRequests < requestsCount) {
            count = totalRequests;
        }
        for (uint32 i = 0; i < count; i++) {
            address account = accountsQueue[i];
            delete accountsQueue[i];
            delete exitRequests[account];
        }
    }

}