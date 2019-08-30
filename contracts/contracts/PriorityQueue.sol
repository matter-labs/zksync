pragma solidity ^0.5.8;

contract PriorityQueue {

    struct RequestCreds {
        bytes20 signatureHash;
        uint expirationBlock;
    }

    // Events
    event NewDepositRequest(
        address indexed sender,
        address indexed toAccount,
        uint16 token,
        uint112 amount,
        bytes20 indexed signatureHash,
        uint expirationBlock
    );

    event NewExitRequest(
        address indexed accountAddress,
        address indexed ethereumAddress,
        uint16 token,
        bytes20 indexed signatureHash,
        uint expirationBlock
    );

    event RemovedRequest(
        bytes20 indexed signatureHash
    );

    // Franklin contract address
    address private franklinAddress;

    // Not satisfied requests
    mapping(uint32 => RequestCreds) public requestsCreds;
    mapping(bytes20 => bool) public requestsExistance;
    uint32 public totalRequests;

    //OPnly Franklin contract permission modifier
    modifier onlyFranklin() {
        require(msg.sender == franklinAddress, "Not the main Franklin contract");
        _;
    }

    // Constructor - sets Franklin contract address
    constructor(address _franklinAddress) public {
        franklinAddress = _franklinAddress;
    }

    // Interface

    function addExitRequest(address accountAddress, address ethereumAddress, uint16 token, bytes20 signatureHash) external {
        require(!requestsExistance[signatureHash], "Exit request from this sender for chosen token exists");
        uint expirationBlock = block.number+250;
        requestsCreds[totalRequests] = RequestCreds(
            signatureHash,
            expirationBlock
        );
        requestsExistance[signatureHash] = true;
        totalRequests++;
        emit NewExitRequest(
            accountAddress,
            ethereumAddress,
            token,
            signatureHash,
            expirationBlock
        );
    }

    function addDepositRequest(address sender, address toAccount, uint16 token, uint112 amount, bytes20 signatureHash) external {
        require(!requestsExistance[signatureHash], "Deposit request from this sender for chosen token and value exists");
        uint expirationBlock = block.number+250;
        requestsCreds[totalRequests] = RequestCreds(
            signatureHash,
            expirationBlock
        );
        requestsExistance[signatureHash] = true;
        totalRequests++;
        emit NewDepositRequest(
            sender,
            toAccount,
            token,
            amount,
            signatureHash,
            expirationBlock
        );
    }

    function removeRequest(bytes20 signatureHash) external onlyFranklin {
        require(requestsExistance[signatureHash], "Exit request from this sender for chosen token doesn't exists");
        delete requestsExistance[signatureHash];
        for (uint32 i = 0; i < totalRequests; i++) {
            if (requestsCreds[i].signatureHash == signatureHash) {
                delete requestsCreds[i];
                for (uint32 j = i; j < totalRequests-1; j++) {
                    requestsCreds[j] = requestsCreds[j+1];
                    delete requestsCreds[j+1];
                }
                break;
            }
        }
        totalRequests--;
        emit RemovedRequest(
            signatureHash
        );
    }

    // Exodus Mode

    function isExodusActivated(uint currentBlock) external view returns (bool) {
        uint expirationBlock = requestsCreds[0].expirationBlock;
        return currentBlock >= expirationBlock;
    }
}