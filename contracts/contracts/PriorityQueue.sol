pragma solidity ^0.5.8;

contract PriorityQueue {

    /// Request credentials structere
    /// Allows to identify (find and delete) the request from mapping
    /// Fields:
    /// - signatureHash - hash of user signature from request
    /// - expirationBlock - number of Ethereum block when request becomes expired
    struct RequestCreds {
        bytes20 signatureHash;
        uint expirationBlock;
    }

    /// New deposit request event
    /// Emitted when deposit request is placed into mapping
    event NewDepositRequest(
        address indexed sender,
        address indexed toAccount,
        uint16 token,
        uint112 amount,
        bytes20 indexed signatureHash,
        uint expirationBlock
    );

    /// New exit request event
    /// Emitted when exit request is placed into mapping
    event NewExitRequest(
        address indexed accountAddress,
        address indexed ethereumAddress,
        uint16 token,
        bytes20 indexed signatureHash,
        uint expirationBlock
    );

    /// Removed request event
    /// Emitted when any request (deposit or exit) is deleted from mapping
    event RemovedRequest(
        bytes20 indexed signatureHash
    );

    /// Franklin contract address
    address private franklinAddress;

    /// Requests credentials mapping (request number - request creds)
    /// Contains creds of unsatisfied requests (deposits and exits). Numbers are in order of requests receiving
    mapping(uint32 => RequestCreds) public requestsCreds;

    /// Requests existance mapping (signature hash - existance flag)
    /// Anyone can get request existance (true if exists) by user signature hash from this request
    mapping(bytes20 => bool) public requestsExistance;

    /// Total number of requests
    uint32 public totalRequests;

    /// Only Franklin contract permission modifier
    modifier onlyFranklin() {
        require(msg.sender == franklinAddress, "Not the main Franklin contract");
        _;
    }

    /// Constructor - sets Franklin contract address
    constructor(address _franklinAddress) public {
        franklinAddress = _franklinAddress;
    }

    /// Add exit request external function
    /// Params:
    /// - accountAddress - address of Franklin account from which the funds must be withdrawn
    /// - ethereumAddress - address of root-chain account to which the funds must be sent
    /// - token - chosen token to withdraw
    /// - signatureHash - user signature hash
    function addExitRequest(
        address accountAddress,
        address ethereumAddress,
        uint16 token,
        bytes20 signatureHash
    ) external {
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

    /// Add deposit request external function
    /// Params:
    /// - sender - address of sender
    /// - toAccount - address of Franklin account to which the funds must be sent
    /// - token - chosen token to deposit
    /// - amount - amount of the chosen token
    /// - signatureHash - user signature hash
    function addDepositRequest(
        address sender,
        address toAccount,
        uint16 token,
        uint112 amount,
        bytes20 signatureHash
    ) external {
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

    /// Remove request external function. Can be used only from Franklin contract
    /// Params:
    /// - signatureHash - user signature hash that is used to identify the request
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

    /// External function to check if there is a need to enter the Exodus mode
    /// Params:
    /// - current Ethereum block
    /// Returns:
    /// - bool flag. True if the Exodus mode must be entered
    function isExodusActivated(uint currentBlock) external view returns (bool) {
        uint expirationBlock = requestsCreds[0].expirationBlock;
        return currentBlock >= expirationBlock;
    }
}