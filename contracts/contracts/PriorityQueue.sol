pragma solidity ^0.5.8;

contract PriorityQueue {

    /// Request credentials structere
    /// Allows to identify (find and delete) the request from mapping
    /// Fields:
    /// - identifier - incremented request identifier
    /// - expirationBlock - the number of Ethereum block when request becomes expired
    struct RequestCreds {
        uint identifier;
        uint expirationBlock;
    }

    /// New request event
    /// Emitted when a request is placed into mapping
    /// Params:
    /// - identifier - request unique identifier
    /// - opType - operation type
    /// - franklinAccountAddress - Franklin address
    /// - ethAddress - Ethereum address
    /// - token - selected token
    /// - amount - the amount of selected token
    /// - signatureHash - the user signature hash
    /// - expirationBlock - the number of Ethereum block when request becomes expired
    event NewRequest(
        uint16 indexed identifier,
        uint8 indexed opType,
        bytes franklinAccountAddress,
        address ethAddress,
        uint16 token,
        uint112 amount,
        bytes20 signatureHash,
        uint indexed expirationBlock
    );

    /// Removed request event
    /// Emitted when a request is deleted from mapping
    event RemovedRequest(
        uint indexed identifier
    );

    /// Franklin contract address
    address private franklinAddress;

    /// Requests credentials mapping (request number - request creds)
    /// Contains creds of unsatisfied requests. Numbers are in order of requests receiving
    mapping(uint => RequestCreds) public requestsCreds;

    /// Requests existance mapping (request identifier - existance flag)
    /// Anyone can get request existance (true if exists) by this request identifier
    mapping(uint => bool) public requestsExistance;

    /// Total number of requests
    uint public totalRequests;

    /// Incremented requests identifier
    uint16 private counter;

    /// Only Franklin contract permission modifier
    modifier onlyFranklin() {
        require(msg.sender == franklinAddress, "Not the main Franklin contract");
        _;
    }

    /// Constructor - sets Franklin contract address
    constructor(address _franklinAddress) public {
        franklinAddress = _franklinAddress;
    }

    /// Add request external function
    /// Params:
    /// - opType - operation type
    /// - franklinAccountAddress - Franklin address
    /// - ethAddress - Ethereum address
    /// - token - the selected token
    /// - amount - the amount of selected token
    /// - signatureHash - the user signature hash
    function addRequest(
        uint8 opType,
        bytes calldata franklinAccountAddress,
        address ethAddress,
        uint16 token,
        uint112 amount,
        bytes20 signatureHash
    ) external {
        require(!requestsExistance[counter], "Request with same identifier exists");
        uint16 identifier = counter;

        uint expirationBlock = block.number+250;

        requestsCreds[totalRequests] = RequestCreds(
            identifier,
            expirationBlock
        );

        requestsExistance[identifier] = true;

        totalRequests++;

        counter = counter == 0xFFFF
            ? 0
            : counter + 1;

        emit NewRequest(
            identifier,
            opType,
            franklinAccountAddress,
            ethAddress,
            token,
            amount,
            signatureHash,
            expirationBlock
        );
    }

    /// Remove request external function. Can be used only from Franklin contract
    /// Params:
    /// - identifier - request identifier
    function removeRequest(uint16 identifier) external onlyFranklin {
        require(requestsExistance[identifier], "This request doesn't exists");
        delete requestsExistance[identifier];
        for (uint32 i = 0; i < totalRequests; i++) {
            if (requestsCreds[i].identifier == identifier) {
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
            identifier
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