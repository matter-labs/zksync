pragma solidity ^0.5.8;

import "openzeppelin-solidity/contracts/ownership/Ownable.sol";

contract ExitQueue is Ownable {

    // Franklin contract address
    address private franklinAddress;

    // Contains not satisfied exit requests
    mapping(uint32 => address) private accountsQueue;
    mapping(address => bytes) private exitRequests; // TODO: -in bytes or struct?
    uint32 public totalRequests;

    constructor(address _franklinAddress) public {
        franklinAddress = _franklinAddress;
    }

    // ExitRequest sctructure
    // TODO: - is it final?
    struct ExitRequest {
        uint32 untilEthBlock;
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

    function getRequests(uint32 _count) external returns (ExitRequest[] memory) {
        require(totalRequests > 0, "No exit requests");
        uint32 requestsToRemove = _count;
        if (_count > totalRequests) {
            requestsToRemove = totalRequests;
        }
        ExitRequest[requestsToRemove] requests = new ExitRequest(requestsToRemove);
        for (uint32 i = 0; i < requestsToRemove; i++) {
            requests[i] = exitRequests[accountsQueue[i]].toExitRequest();
        }
        return requests;
    }

    function toExitRequest(bytes memory publicData) internal returns (ExitRequest) {
        return ExitRequest({
            /// TODO
        });
    }

    function removeRequests(uint32 _count) external {
        require(totalRequests > 0, "No exit requests");
        uint32 requestsToRemove = _count;
        if (_count > totalRequests) {
            requestsToRemove = totalRequests;
        }
        for (uint32 i = 0; i < requestsToRemove; i++) {
            address account = accountsQueue[i];
            delete exitRequests[account];
            delete accountsQueue[i];
        }
        uint32 nonremovedCount = totalRequests - requestsToRemove;
        if (nonremovedCount > 0) {
            for (uint32 i = requestsToRemove; i < totalRequests; i++) {
                accountsQueue[i-requestsToRemove] = accountsQueue[i];
            }
        }
    }

    function checkForExodus() external {
        // TODO: - recode
        if (exitRequests[0].untilEthBlock > block.number) {
            // TODO: - trigger exodus
            
        }
    }

}