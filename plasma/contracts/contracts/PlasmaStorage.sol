// This contract is generated programmatically

pragma solidity ^0.4.24;

// storage variable to later use in delegates chain.
// Also defines all structures
contract PlasmaStorage {

    // For tree depth 24
    bytes32 constant EMPTY_TREE_ROOT = 0x003f7e15e4de3453fe13e11fb4b007f1fce6a5b0f0353b3b8208910143aaa2f7;

    // Plasma itself

    uint256 public constant DEADLINE = 3600;

    event BlockCommitted(uint32 indexed blockNumber);
    event BlockVerified(uint32 indexed blockNumber);

    enum Circuit {
        DEPOSIT,
        TRANSFER,
        EXIT
    }

    enum AccountState {
        NOT_REGISTERED,
        REGISTERED,
        PENDING_EXIT,
        UNCONFIRMED_EXIT
    }

    struct Block {
        uint8 circuit;
        uint64  deadline;
        uint128 totalFees;
        bytes32 newRoot;
        bytes32 publicDataCommitment;
        address prover;
    }

    // Key is block number
    mapping (uint32 => Block) public blocks;
    // Only some addresses can send proofs
    mapping (address => bool) public operators;
    // Fee collection accounting
    mapping (address => uint256) public balances;

    struct Account {
        uint8 state;
        uint32 exitBatchNumber;
        address owner;
        uint256 publicKey;
    }

    // one Ethereum address should have one account
    mapping (address => uint24) public ethereumAddressToAccountID;

    // Plasma account => general information
    mapping (uint24 => Account) public accounts;

    // Public information for users
    bool public stopped;
    uint32 public lastCommittedBlockNumber;
    uint32 public lastVerifiedBlockNumber;
    bytes32 public lastVerifiedRoot;
    uint64 public constant MAX_DELAY = 1 days;
    uint256 public constant DENOMINATOR = 1000000000000;

    // deposits

    uint256 public constant DEPOSIT_BATCH_SIZE = 1;
    uint256 public totalDepositRequests; // enumerates total number of deposit, starting from 0
    uint256 public lastCommittedDepositBatch;
    uint256 public lastVerifiedDepositBatch;
    uint128 public currentDepositBatchFee; // deposit request fee scaled units

    uint24 public constant SPECIAL_ACCOUNT_DEPOSITS = 1;

    uint24 public nextAccountToRegister;

    // some ideas for optimization of the deposit request information storage:
    // store in a mapping: 20k gas to add, 5k to update a record + 5k to update the global counter per batch
    // store in an array: 20k + 5k gas to add, 5k to update + up to DEPOSIT_BATCH_SIZE * SLOAD

    // batch number => (plasma address => deposit information)
    mapping (uint256 => mapping (uint24 => DepositRequest)) public depositRequests;
    mapping (uint256 => DepositBatch) public depositBatches;

    struct DepositRequest {
        uint128 amount;
    }

    enum DepositBatchState {
        CREATED,
        COMMITTED,
        VERIFIED
    }

    struct DepositBatch {
        uint8 state;
        uint24 numRequests;
        uint32 blockNumber;
        uint64 timestamp;
        uint128 batchFee;
    }

    event LogDepositRequest(uint256 indexed batchNumber, uint24 indexed accountID, uint256 indexed publicKey, uint128 amount);
    event LogCancelDepositRequest(uint256 indexed batchNumber, uint24 indexed accountID);

    // Transfers

    uint256 public constant TRANSFER_BLOCK_SIZE = 128;

    // Exits 

    uint256 constant EXIT_BATCH_SIZE = 1;
    uint256 public totalExitRequests; 
    uint256 public lastCommittedExitBatch;
    uint256 public lastVerifiedExitBatch;
    uint128 public currentExitBatchFee; 

    uint24 public constant SPECIAL_ACCOUNT_EXITS = 0;

    // batches for complete exits
    mapping (uint256 => ExitBatch) public exitBatches;

    enum ExitBatchState {
        CREATED,
        COMMITTED,
        VERIFIED
    }

    struct ExitBatch {
        uint8 state;
        uint32 blockNumber;
        uint64 timestamp;
        uint128 batchFee;
    }

    event LogExitRequest(uint256 indexed batchNumber, uint24 indexed accountID);
    event LogCancelExitRequest(uint256 indexed batchNumber, uint24 indexed accountID);

    event LogExit(address indexed ethereumAddress, uint32 indexed blockNumber);
    event LogCompleteExit(address indexed ethereumAddress, uint32 indexed blockNumber, uint24 accountID);

    // mapping ethereum address => block number => balance
    mapping (address => mapping (uint32 => uint128)) public exitAmounts;
    // Delegates chain
    address public transactor;
    address public exitor;
}