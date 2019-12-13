pragma solidity 0.5.10;

import "./Bytes.sol";
import "./Governance.sol";

/// @title Priority Queue Contract
/// @author Matter Labs
contract PriorityQueue {

    /// @notice Rollup contract address
    address internal rollupAddress;

    /// @notice Governance contract
    Governance internal governance;

    /// @notice Deposit operation number
    uint8 constant DEPOSIT_OP = 1;

    /// @notice Full exit operation number
    uint8 constant FULL_EXIT_OP = 6;

    /// @notice Token id bytes length
    uint8 constant TOKEN_BYTES = 2;

    /// @notice Token amount bytes length
    uint8 constant AMOUNT_BYTES = 16;

    /// @notice Ethereum address bytes length
    uint8 constant ADDR_BYTES = 20;

    /// @notice Rollup account id bytes length
    uint8 constant ACC_NUM_BYTES = 3;

    /// @notice Rollup nonce bytes length
    uint8 constant NONCE_BYTES = 4;

    /// @notice Signature (for example full exit signature) length
    uint8 constant SIGNATURE_BYTES = 64;

    /// @notice Public key length
    uint8 constant PUBKEY_BYTES = 32;

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

    /// @notice Sets rollup address
    /// param _rollupAddress Address of the Rollup contract
    function setRollupAddress(address _rollupAddress) external {
        // Its possible to set rollup contract address only if it has not been setted before
        require(
            rollupAddress == address(0),
            "pcs11"
        ); // pcs11 - frankin address is already setted
        
        // Check for governor
        governance.requireGovernor(msg.sender);

        // Set rollup address
        rollupAddress = _rollupAddress;
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
        requireRollup();
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

    /// @notice Collects a fee from provided requests number for the validator, return it and delete these requests
    /// @param _number The number of requests to process
    function collectValidatorsFeeAndDeleteRequests(uint64 _number) external returns (uint256) {
        requireRollup();
        require(
            _number <= totalOpenPriorityRequests,
            "pecs21"
        ); // pecs21 - number is heigher than total priority requests number

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

    /// @notice Returns open deposit requests concated pubdata
    function getOutstandingDeposits() external view returns (bytes memory depositsPubData) {
        for (uint64 i = firstPriorityRequestId; i < firstPriorityRequestId + totalOpenPriorityRequests; i++) {
            if (priorityRequests[i].opType == DEPOSIT_OP) {
                depositsPubData = Bytes.concat(depositsPubData, priorityRequests[i].pubData);
            }
        }
    }

    /// @notice Compares Rollup operation with corresponding priority requests' operation
    /// @param _opType Operation type
    /// @param _pubData Operation pub data
    /// @param _id - Request id
    function isPriorityOpValid(uint8 _opType, bytes calldata _pubData, uint64 _id) external view returns (bool) {
        uint64 _priorityRequestId = _id + firstPriorityRequestId + totalCommittedPriorityRequests;
        bytes memory priorityPubData;
        bytes memory onchainPubData;
        if (_opType == DEPOSIT_OP && priorityRequests[_priorityRequestId].opType == DEPOSIT_OP) {
            priorityPubData = Bytes.slice(priorityRequests[_priorityRequestId].pubData, ADDR_BYTES, ADDR_BYTES + AMOUNT_BYTES + TOKEN_BYTES);
            onchainPubData = _pubData;
        } else if (_opType == FULL_EXIT_OP && priorityRequests[_priorityRequestId].opType == FULL_EXIT_OP) {
            priorityPubData = Bytes.slice(priorityRequests[_priorityRequestId].pubData, 0, ACC_NUM_BYTES + PUBKEY_BYTES + ADDR_BYTES + TOKEN_BYTES + NONCE_BYTES + SIGNATURE_BYTES);
            onchainPubData = Bytes.slice(_pubData, 0, ACC_NUM_BYTES + PUBKEY_BYTES + ADDR_BYTES + TOKEN_BYTES + NONCE_BYTES + SIGNATURE_BYTES);
        } else {
            revert("peid11"); // peid11 - wrong operation
        }
        return (priorityPubData.length > 0) &&
            (keccak256(onchainPubData) == keccak256(priorityPubData));
    }

    /// @notice Checks if provided number is less than uncommitted requests count
    /// @param _number Number of requests
    function validateNumberOfRequests(uint64 _number) external view {
        require(
            _number <= totalOpenPriorityRequests-totalCommittedPriorityRequests,
            "pevs11"
        ); // pevs11 - too much priority requests
    }

    /// @notice Increases committed requests count by provided number
    /// @param _number Number of requests
    function increaseCommittedRequestsNumber(uint64 _number) external {
        requireRollup();
        totalCommittedPriorityRequests += _number;
    }

    /// @notice Decreases committed requests count by provided number
    /// @param _number Number of requests
    function decreaseCommittedRequestsNumber(uint64 _number) external {
        requireRollup();
        totalCommittedPriorityRequests -= _number;
    }

    /// @notice Checks if Exodus mode must be entered and returns bool.
    /// @dev Returns bool flag that is true if the Exodus mode must be entered.
    /// @dev Exodus mode must be entered in case of current ethereum block number is higher than the oldest
    /// @dev of existed priority requests expiration block number.
    function triggerExodusIfNeeded() external view returns (bool) {
        return block.number >= priorityRequests[firstPriorityRequestId].expirationBlock &&
            priorityRequests[firstPriorityRequestId].expirationBlock != 0;
    }

    /// @notice Check if the sender is rollup contract
    function requireRollup() internal view {
        require(
            msg.sender == rollupAddress,
            "prn11"
        ); // prn11 - only by rollup
    }
}