pragma solidity ^0.5.8;

contract PriorityQueue {

    /// Ethereum expiration blocks delta
    uint private expiration_delta = 250;

    address private owner;
    /// Franklin contract address
    address private franklinAddress;

    /// New request event
    /// Emitted when a request is placed into mapping
    /// Params:
    /// - pubData - request data
    /// - expirationBlock - the number of Ethereum block when request becomes expired
    event NewRequest(
        bytes pubData,
        uint indexed expirationBlock
    );

    /// Requests expiration mapping (request id - expiration block)
    /// Contains expiration block of unsatisfied requests. Numbers are in order of requests receiving
    mapping(uint => uint) public requestsExpiration;
    /// Total number of requests
    uint public totalRequests;

    /// Only Franklin contract permission
    function requireFranklin() internal view {
        require(msg.sender == franklinAddress, "Not the main Franklin contract");
    }

    /// Only Owner permission
    function requireOwner() internal view {
        require(msg.sender == owner, "Not the contract owner");
    }

    /// Constructor - sets owner
    constructor() public {
        owner = msg.sender;
    }

    /// Changes Franklin contract address. Only by onwner
    function changeFranlkinContractAddress(address _franklinAddress) public {
        requireOwner();
        franklinAddress = _franklinAddress;
    }

    /// Changes expiration delta. Only by onwner
    function changeFranlkinContractAddress(uint _expiration_delta) public {
        requireOwner();
        expiration_delta = _expiration_delta;
    }

    /// Add request. Can be used only from Franklin contract
    /// Params:
    /// - pubData - request data
    function addRequest(bytes calldata pubData) external {
        requireFranklin();

        uint expirationBlock = block.number + EXPIRATION_DELTA;
        requestsExpiration[totalRequests] = expirationBlock;
        totalRequests++;

        emit NewRequest(
            pubData,
            expirationBlock
        );
    }

    /// Removes executed requests. Can be used only from Franklin contract
    /// Params:
    /// - count - number of executed requests
    function executeRequests(uint count) external {
        requireFranklin();
        require(totalRequests >= count, "Count of executed requests is higher than their count");

        for (uint i = 0; i < totalRequests; i++) {
            if (i >= count) {
                requestsExpiration[i-count] = requestsExpiration[i];
            }
            delete requestsExpiration[i];
        }
        totalRequests--;
    }

    /// External function to check if there is a need to enter the Exodus mode
    /// Params:
    /// - current Ethereum block
    /// Returns:
    /// - bool flag. True if the Exodus mode must be entered
    function isExodusActivated(uint currentBlock) external view returns (bool) {
        uint expirationBlock = requestsExpiration[0];
        return currentBlock >= expirationBlock;
    }
}