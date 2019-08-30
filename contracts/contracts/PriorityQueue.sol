pragma solidity ^0.5.8;
pragma experimental ABIEncoderV2;

contract PriorityQueue {

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

    event RemovedDepositRequest(
        bytes20 indexed signatureHash
    );

    event RemovedExitRequest(
        bytes20 indexed signatureHash
    );

    // Franklin contract address
    address private franklinAddress;

    // Not satisfied requests
    mapping(uint32 => RequestCreds) public exitRequestsCreds;
    mapping(bytes20 => ExitRequest) public exitRequests;
    uint32 public totalExitRequests;


    mapping(uint32 => RequestCreds) public depositRequestsCreds;
    mapping(bytes20 => DepositRequest) public depositRequests;
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

    // Exit Queue

    function addExitRequest(address accountAddress, address ethereumAddress, uint16 token, bytes20 signatureHash) external {
        require(exitRequests[signatureHash].expirationBlock == 0, "Exit request from this sender for chosen token exists");

        exitRequestsCreds[totalExitRequests] = RequestCreds(
            signatureHash
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

    // Deposit Queue

    function addDepositRequest(address sender, address toAccount, uint16 token, uint112 amount, bytes20 signatureHash) external {
        require(depositRequests[signatureHash].expirationBlock == 0, "Deposit request from this sender for chosen token and value exists");

        depositRequestsCreds[totalDepositRequests] = RequestCreds(
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