pragma solidity 0.5.10;

import "./Bytes.sol";

contract PriorityQueue {

    address internal franklinAddress; // Franklin contract address
    address internal ownerAddress; // Owner address
    
    // Priority operation numbers
    uint8 constant DEPOSIT_OP = 1; // Deposit operation number
    uint8 constant FULL_EXIT_OP = 6; // Full exit operation number

    // Operation fields bytes lengths
    uint8 constant TOKEN_BYTES = 2; // token id
    uint8 constant AMOUNT_BYTES = 16; // token amount
    uint8 constant ETH_ADDR_BYTES = 20; // ethereum address
    uint8 constant ACC_NUM_BYTES = 3; // franklin account id
    uint8 constant NONCE_BYTES = 4; // franklin nonce

    // Franklin chain address length
    uint8 constant PUBKEY_HASH_LEN = 20;
    // Signature (for example full exit signature) length
    uint8 constant SIGNATURE_LEN = 64;
    // Public key length
    uint8 constant PUBKEY_LEN = 32;
    // Expiration delta for priority request to be satisfied (in ETH blocks)
    uint256 constant PRIORITY_EXPIRATION = 250; // About 1 hour

    // New priority request event
    // Emitted when a request is placed into mapping
    // Params:
    // - opType - operation type
    // - pubData - operation data
    // - expirationBlock - the number of Ethereum block when request becomes expired
    // - fee - validators' fee
    event NewPriorityRequest(
        uint64 serialId,
        uint8 opType,
        bytes pubData,
        uint256 expirationBlock,
        uint256 fee
    );

    // Priority Operation contains operation type, its data, expiration block, and fee
    struct PriorityOperation {
        uint8 opType;
        bytes pubData;
        uint256 expirationBlock;
        uint256 fee;
    }

    // Priority Requests mapping (request id - operation)
    // Contains op type, pubdata, fee and expiration block of unsatisfied requests.
    // Numbers are in order of requests receiving
    mapping(uint64 => PriorityOperation) public priorityRequests;
    // First priority request id
    uint64 public firstPriorityRequestId;
    // Total number of requests
    uint64 public totalOpenPriorityRequests;
    // Total number of committed requests
    uint64 public totalCommittedPriorityRequests;

    // Constructor sets owner address
    constructor(address _ownerAddress) public {
        ownerAddress = _ownerAddress;
    }

    // Change franklin address
    // _franklinAddress - address of the Franklin contract
    function changeFranklinAddress(address _franklinAddress) external {
        // Its possible to set franklin contract address only if it has not been setted before
        require(
            franklinAddress == address(0),
            "pcs11"
        ); // pcs11 - frankin address is already setted
        requireOwner();
        franklinAddress = _franklinAddress;
    }

    // Calculate expiration block for request, store this request and emit NewPriorityRequest event
    // Params:
    // - _opType - priority request type
    // - _fee - validators' fee
    // - _pubData - request data
    function addPriorityRequest(
        uint8 _opType,
        uint256 _fee,
        bytes calldata _pubData
    ) external {
        requireFranklin();
        // Expiration block is: current block number + priority expiration delta
        uint256 expirationBlock = block.number + PRIORITY_EXPIRATION;

        priorityRequests[firstPriorityRequestId+totalOpenPriorityRequests] = PriorityOperation({
            opType: _opType,
            pubData: _pubData,
            expirationBlock: expirationBlock,
            fee: _fee
        });

        emit NewPriorityRequest(
            firstPriorityRequestId+totalOpenPriorityRequests,
            _opType,
            _pubData,
            expirationBlock,
            _fee
        );

        totalOpenPriorityRequests++;
    }

    // Collects a fee from provided requests number for the validator, return it and delete these requests
    // Params:
    // - _number - the number of requests
    function collectValidatorsFeeAndDeleteRequests(uint64 _number) external returns (uint256) {
        requireFranklin();
        require(
            _number <= totalOpenPriorityRequests,
            "pcs21"
        ); // pcs21 - number is heigher than total priority requests number

        uint256 totalFee = 0;
        for (uint64 i = firstPriorityRequestId; i < firstPriorityRequestId + _number; i++) {
            totalFee += priorityRequests[i].fee;
            delete priorityRequests[i];
        }
        totalOpenPriorityRequests -= _number;
        firstPriorityRequestId += _number;
        totalCommittedPriorityRequests -= _number;

        return totalFee;
    }

    // Returns deposits pub data in bytes
    function getOutstandingDeposits() external view returns (bytes memory depositsPubData) {
        for (uint64 i = firstPriorityRequestId; i < firstPriorityRequestId + totalOpenPriorityRequests; i++) {
            if (priorityRequests[i].opType == DEPOSIT_OP) {
                depositsPubData = Bytes.concat(depositsPubData, priorityRequests[i].pubData);
            }
        }
    }

    // Compares operation with corresponding priority requests' operation
    // Params:
    // - _opType - operation type
    // - _pubData - operation pub data
    // - _id - operation number
    function isPriorityOpValid(uint8 _opType, bytes calldata _pubData, uint64 _id) external view returns (bool) {
        uint64 _priorityRequestId = _id + firstPriorityRequestId + totalCommittedPriorityRequests;
        bytes memory priorityPubData;
        bytes memory onchainPubData;
        if (_opType == DEPOSIT_OP && priorityRequests[_priorityRequestId].opType == DEPOSIT_OP) {
            priorityPubData = Bytes.slice(priorityRequests[_priorityRequestId].pubData, ETH_ADDR_BYTES, PUBKEY_HASH_LEN + AMOUNT_BYTES + TOKEN_BYTES);
            onchainPubData = _pubData;
        } else if (_opType == FULL_EXIT_OP && priorityRequests[_priorityRequestId].opType == FULL_EXIT_OP) {
            priorityPubData = Bytes.slice(priorityRequests[_priorityRequestId].pubData, 0, PUBKEY_LEN + ETH_ADDR_BYTES + TOKEN_BYTES + NONCE_BYTES + SIGNATURE_LEN);
            onchainPubData = Bytes.slice(_pubData, ACC_NUM_BYTES, PUBKEY_LEN + ETH_ADDR_BYTES + TOKEN_BYTES + NONCE_BYTES + SIGNATURE_LEN);
        } else {
            revert("pid11"); // pid11 - wrong operation
        }
        return (priorityPubData.length > 0) &&
            (keccak256(onchainPubData) == keccak256(priorityPubData));
    }

    // Checks if provided number is less than uncommitted requests count
    // Params:
    // - _number - number of requests
    function validateNumberOfRequests(uint64 _number) external view {
        require(
            _number <= totalOpenPriorityRequests-totalCommittedPriorityRequests,
            "pvs11"
        ); // pvs11 - too much priority requests
    }

    // Increases committed requests count by provided number
    function increaseCommittedRequestsNumber(uint64 _number) external {
        requireFranklin();
        totalCommittedPriorityRequests += _number;
    }

    // Decreases committed requests count by provided number
    function decreaseCommittedRequestsNumber(uint64 _number) external {
        requireFranklin();
        totalCommittedPriorityRequests -= _number;
    }

    // Checks if Exodus mode must be entered and returns bool.
    // Returns bool flag that is true if the Exodus mode must be entered.
    // Exodus mode must be entered in case of current ethereum block number is higher than the oldest
    // of existed priority requests expiration block number.
    function triggerExodusIfNeeded() external view returns (bool) {
        return block.number >= priorityRequests[firstPriorityRequestId].expirationBlock &&
            priorityRequests[firstPriorityRequestId].expirationBlock != 0;
    }

    // Check if the sender is franklin contract
    function requireFranklin() internal view {
        require(
            msg.sender == franklinAddress,
            "prn11"
        ); // prn11 - only by franklin
    }

    // Check if the sender is owner
    function requireOwner() internal view {
        require(
            msg.sender == ownerAddress,
            "prr11"
        ); // prr11 - only by owner
    }
}