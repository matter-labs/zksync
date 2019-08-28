pragma solidity ^0.5.8;
pragma experimental ABIEncoderV2;

contract ExitQueue {

    //TODO
    uint256 constant requestLength = ;

    struct ExitRequest {
        address accountAddress;
        address ethereumAddress;
        uint16 token;
        bytes20 signature;
        uint expirationBlock;
    }

    struct RequestCreds {
        address accountAddress;
        uint16 token;
    }

    // Franklin contract address
    address private franklinAddress;

    // Contains not satisfied exit requests
    mapping(uint32 => RequestCreds) private exitRequestsCreds;
    mapping(address => mapping(uint16 => ExitRequest)) private exitRequests;
    uint32 public totalRequests;

    modifier onlyFranklin() {
        require(msg.sender == franklinAddress, "Not the main Franklin contract");
        _;
    }

    constructor(address _franklinAddress) public {
        franklinAddress = _franklinAddress;
    }

    function validateSignature(address accountAddress, bytes memory signature) internal returns (bytes20 pedersenHash) {
        // TODO
    }

    function requestToBytes(ExitRequest memory request) internal returns (bytes memory requestBytes) {
        // TODO
    }

    function addRequest(address accountAddress, address ethereumAddress, uint16 token, bytes calldata signature) external {
        require(exitRequests[accountAddress][token].expirationBlock == 0, "Exit request from this sender for chosen token exists");

        bytes20 signatureHash = validateSignature(accountAddress, signature);

        exitRequestsCreds[totalRequests] = RequestCreds(
            accountAddress,
            token
        );
        exitRequests[accountAddress][token] = ExitRequest(
            accountAddress,
            ethereumAddress,
            token,
            signatureHash,
            block.number+250
        );
        totalRequests++;
    }

    function getRequests(uint32 _count) external view returns (bytes memory) {
        require(totalRequests > 0, "No exit requests");

        uint32 requestsToGet = _count;
        if (_count > totalRequests) {
            requestsToGet = totalRequests;
        }

        bytes memory requests = new bytes(requestsToGet*requestLength);
        for (uint32 i = 0; i < requestsToGet; i++) {
            RequestCreds memory creds = exitRequestsCreds[i];
            // TODO
            requests.concat(exitRequests[creds.accountAddress][creds.token].requestToBytes());
        }
        return requests;
    }

    function removeRequests(uint32 _count) external onlyFranklin {
        require(totalRequests > 0, "No exit requests");

        uint32 requestsToRemove = _count;
        if (_count > totalRequests) {
            requestsToRemove = totalRequests;
        }

        for (uint32 i = 0; i < requestsToRemove; i++) {
            RequestCreds memory creds = exitRequestsCreds[i];
            delete exitRequestsCreds[i];
            delete exitRequests[creds.accountAddress][creds.token];
        }

        uint32 nonremovedCount = totalRequests - requestsToRemove;
        if (nonremovedCount > 0) {
            for (uint32 i = requestsToRemove; i < totalRequests; i++) {
                exitRequestsCreds[i-requestsToRemove] = exitRequestsCreds[i];
                delete exitRequestsCreds[i];
            }
        }

        totalRequests -= _count;
    }

    function isExodusActivated(uint currentBlock) external view returns (bool) {
        uint expirationBlock = exitRequests[exitRequestsCreds[0].accountAddress][exitRequestsCreds[0].token].expirationBlock;
        return currentBlock >= expirationBlock;
    }

}