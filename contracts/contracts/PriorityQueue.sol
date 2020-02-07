pragma solidity 0.5.16;

import "./Bytes.sol";
import "./Governance.sol";

/// @title Priority Queue Contract
/// @author Matter Labs
contract PriorityQueue {

    /// @notice Rollup contract address
    address internal franklinAddress;

    /// @notice Governance contract
    Governance internal governance;
    
    /// @notice Deposit operation number
    uint8 constant DEPOSIT_OP = 1;

    /// @notice Full exit operation number
    uint8 constant FULL_EXIT_OP = 6;

    /// @notice ChangePubKeyPriority operation number
    uint8 constant CHANGE_PUBKEY_PRIORITY = 8;

    /// @notice Token id bytes length
    uint8 constant TOKEN_BYTES = 2;

    /// @notice Token amount bytes length
    uint8 constant AMOUNT_BYTES = 16;

    /// @notice Ethereum address bytes length
    uint8 constant ETH_ADDR_BYTES = 20;

    /// @notice Rollup account id bytes length
    uint8 constant ACC_NUM_BYTES = 3;

    /// @notice success flag length
    uint8 constant SUCCESS_FLAG_BYTES = 1;


    /// @notice Rollup nonce bytes length
    uint8 constant NONCE_BYTES = 4;

    /// @notice Franklin chain address length
    uint8 constant PUBKEY_HASH_BYTES = 20;

    /// @notice Expiration delta for priority request to be satisfied (in ETH blocks)
    uint256 constant PRIORITY_EXPIRATION = 4 * 60 * 24; // One day

    /// @notice New priority request event. Emitted when a request is placed into mapping
    event NewPriorityRequest(
        uint64 serialId,
        uint8 opType,
        bytes pubData,
        uint256 expirationBlock,
        uint256 fee
    );

    /// @notice Priority Operation container
    /// @member opType Priority operation type
    /// @member pubData Priority operation public data
    /// @member expirationBlock Expiration block number (ETH block) for this request (must be satisfied before)
    /// @member fee Validators fee
    struct PriorityOperation {
        uint8 opType;
        bytes pubData;
        uint256 expirationBlock;
        uint256 fee;
    }

    /// @notice Priority Requests mapping (request id - operation)
    /// @dev Contains op type, pubdata, fee and expiration block of unsatisfied requests.
    /// @dev Numbers are in order of requests receiving
    mapping(uint64 => PriorityOperation) public priorityRequests;

    /// @notice First open priority request id
    uint64 public firstPriorityRequestId;

    /// @notice Total number of requests
    uint64 public totalOpenPriorityRequests;

    /// @notice Total number of committed requests.
    /// @dev Used in checks: if the request matches the operation on Rollup contract and if provided number of requests is not too big
    uint64 public totalCommittedPriorityRequests;

    /// @notice Constructs PriorityQueue contract
    /// @param _governanceAddress Governance contract address
    constructor(address _governanceAddress) public {
        governance = Governance(_governanceAddress);
    }

    /// @notice Sets rollup address if it has not been set before
    /// @param _franklinAddress Address of the Rollup contract
    function setFranklinAddress(address _franklinAddress) external {
        // Its possible to set franklin contract address only if it has not been setted before
        require(
            franklinAddress == address(0),
            "pcs11"
        ); // pcs11 - franklin address is already setted
        // Check for governor
        governance.requireGovernor(msg.sender);
        // Set franklin address
        franklinAddress = _franklinAddress;
    }

    /// @notice Saves priority request in storage
    /// @dev Calculates expiration block for request, store this request and emit NewPriorityRequest event
    /// @param _opType Rollup operation type
    /// @param _fee Validators' fee
    /// @param _pubData Operation pubdata
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

    /// @notice Collect a fee from provided requests number for the validator and delete these requests
    /// @param _number The number of requests to process
    /// @return validators fee
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

    /// @notice Concates open (outstanding) deposit requests public data up to defined deposits number
    /// @dev Deletes processed requests.
    /// @param _number Supposed number of open requests to look at and delete
    /// @return concated deposits public data for limited number of deposits so as not to go beyond the block gas limit in the caller function
    function deletePriorityRequestsAndPopOutstandingDeposits(uint64 _number) external returns (bytes memory depositsPubData) {
        requireFranklin();
        require(
            totalOpenPriorityRequests > 0,
            "pgs11"
        ); // pgs11 - no one priority request left
        uint64 toProcess = totalOpenPriorityRequests < _number ? totalOpenPriorityRequests : _number;
        for (uint64 i = 0; i < toProcess; i++) {
            uint64 id = firstPriorityRequestId + i;
            if (priorityRequests[id].opType == DEPOSIT_OP) {
                depositsPubData = Bytes.concat(depositsPubData, priorityRequests[id].pubData);
            }
            delete priorityRequests[id];
        }
        firstPriorityRequestId += toProcess;
        totalOpenPriorityRequests -= toProcess;
    }

    /// @notice Compares Rollup operation with corresponding priority requests' operation
    /// @param _opType Operation type
    /// @param _pubData Operation pub data
    /// @param _id - Request id
    /// @return bool flag that indicates if priority operation is valid (exists in priority requests list on the specified place)
    function isPriorityOpValid(uint8 _opType, bytes calldata _pubData, uint64 _id) external view returns (bool) {
        uint64 _priorityRequestId = _id + firstPriorityRequestId + totalCommittedPriorityRequests;
        uint8 priorReqType = priorityRequests[_priorityRequestId].opType;
        bytes memory priorReqPubdata = priorityRequests[_priorityRequestId].pubData;

        require(priorReqType == _opType, "pid10"); // pid10 - incorrect priority op type

        bytes memory cmpPriorityQueueBytes;
        bytes memory cmpOpCommittedBytes;
        if (_opType == DEPOSIT_OP) {
            // we don't know account if of the receiver when we create priority queue request
            // that's why we ignore it here
            uint comparePubdataLen = TOKEN_BYTES + AMOUNT_BYTES + ETH_ADDR_BYTES;

            // deposit pubdata contains ETH address of the sender, that is why we have `ETH_ADDR_BYTES` as an offset
            cmpPriorityQueueBytes = Bytes.slice(priorReqPubdata, ETH_ADDR_BYTES, comparePubdataLen);
            cmpOpCommittedBytes = Bytes.slice(_pubData, 0, comparePubdataLen);

        } else if (_opType == FULL_EXIT_OP) {
            // we don't know full exit amount when we create full exit request, that why full amount is ignored here
            uint comparePubdataLen = ACC_NUM_BYTES + ETH_ADDR_BYTES + TOKEN_BYTES;

            cmpPriorityQueueBytes = Bytes.slice(priorReqPubdata, 0, comparePubdataLen);
            cmpOpCommittedBytes = Bytes.slice(_pubData, 0, comparePubdataLen);

        } else if (_opType == CHANGE_PUBKEY_PRIORITY) {
            uint comparePubdataLen = PUBKEY_HASH_BYTES + ETH_ADDR_BYTES;

            cmpPriorityQueueBytes = Bytes.slice(priorReqPubdata, 0, comparePubdataLen);
            // we don't know account id and success flag when we create priority operation that why
            // we ignore this field from pubdata (using offset)
            cmpOpCommittedBytes = Bytes.slice(_pubData, ACC_NUM_BYTES + SUCCESS_FLAG_BYTES, comparePubdataLen);
        } else {
            revert("pid11");
            // pid11 - wrong operation
        }
        return (cmpPriorityQueueBytes.length > 0) &&
        (keccak256(cmpOpCommittedBytes) == keccak256(cmpPriorityQueueBytes));
    }

    /// @notice Checks if provided number is less than uncommitted requests count
    /// @param _number Number of requests
    function validateNumberOfRequests(uint64 _number) external view {
        require(
            _number <= totalOpenPriorityRequests-totalCommittedPriorityRequests,
            "pvs11"
        ); // pvs11 - too much priority requests
    }

    /// @notice Increases committed requests count by provided number
    /// @param _number Number of requests
    function increaseCommittedRequestsNumber(uint64 _number) external {
        requireFranklin();
        totalCommittedPriorityRequests += _number;
    }

    /// @notice Decreases committed requests count by provided number
    /// @param _number Number of requests
    function decreaseCommittedRequestsNumber(uint64 _number) external {
        requireFranklin();
        totalCommittedPriorityRequests -= _number;
    }

    /// @notice Checks if Exodus mode must be entered.
    /// @dev Exodus mode must be entered in case of current ethereum block number is higher than the oldest
    /// @dev of existed priority requests expiration block number.
    /// @return bool flag that indicates if exodus mode must be entered.
    function triggerExodusIfNeeded() external view returns (bool) {
        return block.number >= priorityRequests[firstPriorityRequestId].expirationBlock &&
            priorityRequests[firstPriorityRequestId].expirationBlock != 0;
    }

    /// @notice Check if the sender is rollup contract
    function requireFranklin() internal view {
        require(
            msg.sender == franklinAddress,
            "prn11"
        ); // prn11 - only by franklin
    }
}
