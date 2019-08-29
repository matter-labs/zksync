pragma solidity ^0.5.8;
pragma experimental ABIEncoderV2;

contract PriorityQueue {

    // Constants
    uint public constant depositRequestLength = 97;
    uint public constant exitRequestLength = 94;

    // Structs
    struct DepositRequest {
        address sender; // 20 bytes
        address toAccount; // 20 bytes
        uint16 token; // 2 bytes
        uint112 amount; // 3 bytes
        bytes20 signatureHash; // 20 bytes
        uint expirationBlock; // 32 bytes
    }

    struct ExitRequest {
        address accountAddress; // 20 bytes
        address ethereumAddress; // 20 bytes
        uint16 token; // 2 bytes
        bytes20 signatureHash; // 20 bytes
        uint expirationBlock; // 32 bytes
    }

    struct RequestCreds {
        bytes20 signatureHash;
    }

    // Franklin contract address
    address private franklinAddress;

    // Contains not satisfied requests
    mapping(uint32 => RequestCreds) private exitRequestsCreds;
    mapping(bytes20 => ExitRequest) private exitRequests;
    uint32 public totalExitRequests;


    mapping(uint32 => RequestCreds) private depositRequestsCreds;
    mapping(bytes20 => DepositRequest) private depositRequests;
    uint32 public totalDepositRequests;

    //OPnly Franklin contract permission modifier
    modifier onlyFranklin() {
        require(msg.sender == franklinAddress, "Not the main Franklin contract");
        _;
    }
    
    // Constructor - sets Franklin contract address
    constructor(address _franklinAddress) public {
        franklinAddress = _franklinAddress;
    }

    // Helpers
    function validateDepositSignature(DepositRequest memory request) internal returns (bytes20 pedersenHash) {
        // TODO
    }

    function validateExitSignature(address accountAddress, address ethereumAddress, uint16 token, bytes calldata signature) internal returns (bytes20 pedersenHash) {
        // TODO
    }

    function exitRequestToBytes(ExitRequest memory request) internal returns (bytes memory requestBytes) {
        // TODO
    }

    function depositRequestToBytes(DepositRequest memory request) internal returns (bytes memory requestBytes) {
        // TODO
    }

    // Exit Queue

    function addExitRequest(address accountAddress, address ethereumAddress, uint16 token, bytes calldata signature) external {
        bytes20 signatureHash = validateExitSignature(accountAddress, ethereumAddress, token, signature);
        require(exitRequests[signatureHash].expirationBlock == 0, "Exit request from this sender for chosen token exists");

        exitRequestsCreds[totalExitRequests] = ExitRequestCreds(
            accountAddress,
            token
        );
        exitRequests[signatureHash] = ExitRequest(
            accountAddress,
            ethereumAddress,
            token,
            signatureHash,
            block.number+250
        );
        totalExitRequests++;
    }

    function getExitRequests(uint32 _count) external view returns (bytes memory) {
        require(totalExitRequests > 0, "No exit requests");

        uint32 requestsToGet = _count;
        if (_count > totalExitRequests) {
            requestsToGet = totalExitRequests;
        }

        bytes memory requests = new bytes(requestsToGet*exitRequestLength);
        for (uint32 i = 0; i < requestsToGet; i++) {
            RequestCreds memory creds = exitRequestsCreds[i];
            // TODO
            requests.concat(exitRequests[creds.signatureHash].exitRequestToBytes());
        }
        return requests;
    }

    function removeExitRequest(bytes20 signatureHash) external onlyFranklin {
        require(exitRequests[signatureHash].expirationBlock != 0, "Exit request from this sender for chosen token doesn't exists");
        
        delete exitRequests[signatureHash];

        for (uint32 i = 0; i < totalExitRequests; i++) {
            if (exitRequestsCreds[i].signatureHash == signatureHash) {
                delete exitRequestsCreds[i];
                for (uint32 j = i; j < totalExitRequests-1; j++) {
                    exitRequestsCreds[j] = exitRequestsCreds[j+1];
                    delete exitRequestsCreds[j+1];
                }
                break;
            }
        }

        totalExitRequests--;
    }

    // function removeRequests(uint32 _count) external onlyFranklin {
    //     require(totalRequests > 0, "No exit requests");

    //     uint32 requestsToRemove = _count;
    //     if (_count > totalRequests) {
    //         requestsToRemove = totalRequests;
    //     }

    //     for (uint32 i = 0; i < requestsToRemove; i++) {
    //         RequestCreds memory creds = exitRequestsCreds[i];
    //         delete exitRequestsCreds[i];
    //         delete exitRequests[creds.accountAddress][creds.token];
    //     }

    //     uint32 nonremovedCount = totalRequests - requestsToRemove;
    //     if (nonremovedCount > 0) {
    //         for (uint32 i = requestsToRemove; i < totalRequests; i++) {
    //             exitRequestsCreds[i-requestsToRemove] = exitRequestsCreds[i];
    //             delete exitRequestsCreds[i];
    //         }
    //     }

    //     totalRequests -= _count;
    // }

    // Deposit Queue

    function addDepositRequest(address sender, address toAccount, uint16 token, uint112 amount, bytes calldata signature) external {
        bytes20 signatureHash = validateDepositSignature(sender, toAccount, token, amount, signature);
        require(depositRequests[signatureHash].expirationBlock == 0, "Deposit request from this sender for chosen token and value exists");

        depositRequestsCreds[totalDepositRequests] = DepositRequestCreds(
            signatureHash
        );
        depositRequests[signatureHash] = DepositRequest(
            sender,
            toAccount,
            token,
            amount,
            signatureHash,
            block.number+250
        );
        totalDepositRequests++;
    }

    function getDeposiRequests(uint32 _count) external view returns (bytes memory) {
        require(totalDepositRequests > 0, "No deposit requests");

        uint32 requestsToGet = _count;
        if (_count > totalDepositRequests) {
            requestsToGet = totalDepositRequests;
        }

        bytes memory requests = new bytes(requestsToGet*depositRequestLength);
        for (uint32 i = 0; i < requestsToGet; i++) {
            RequestCreds memory creds = depositRequestsCreds[i];
            // TODO
            requests.concat(depositRequests[creds.signatureHash].depositRequestToBytes());
        }
        return requests;
    }

    function removeDepositRequest(bytes20 signatureHash) external onlyFranklin {
        require(depositRequests[signatureHash].expirationBlock != 0, "Deposit request from this sender for chosen token and value doesn't exists");
        
        delete depositRequests[signatureHash];

        for (uint32 i = 0; i < totalDepositRequests; i++) {
            if (exitRequestsCreds[i].signatureHash == signatureHash) {
                delete exitRequestsCreds[i];
                for (uint32 j = i; j < totalDepositRequests-1; j++) {
                    exitRequestsCreds[j] = exitRequestsCreds[j+1];
                    delete exitRequestsCreds[j+1];
                }
                break;
            }
        }

        totalDepositRequests--;
    }

    // Exodus Mode

    function isExodusActivated(uint currentBlock) external view returns (bool) {
        uint expirationExitBlock = exitRequests[exitRequestsCreds[0].signatureHash].expirationBlock;
        uint expirationDepositBlock = depositRequests[depositRequestsCreds[0].signatureHash].expirationBlock;
        return currentBlock >= expirationExitBlock || currentBlock >= expirationDepositBlock;
    }

}