pragma solidity ^0.5.8;


contract Franklin {

    // Address which will excercise governance over the network
    // i.e. add tokens, change validator set, conduct upgrades
    address public networkGovernance;

    // Address of the account which is allowed to trigger exodus mode
    // (mass exits in the case that censorship resistance has failed)
    address public exitQueue;

    // Total number of ERC20 tokens registered in the network (excluding ETH, which is hardcoded as tokenId = 0)
    uint totalTokens;

    // List of registered tokens by tokenId
    mapping (uint32 => address) public tokens;

    // Root-chain balance: users can send funds from and to Franklin only from these balances
    struct Balance {
        uint112 balance;

        // Locked amount is necessary for deposits, see docs
        uint112 amountLocked;

        // Locked amount becomes free at ETH blockNumber = lockedUntilBlock
        uint32  lockedUntilBlock;
    }

    // List of balances per owner and tokenId
    mapping (address => mapping (uint32 => Balance)) public balances;

    // Type of block processing operation
    enum OpType {
        Deposit,
        Withdraw
    }

    // Operations are required to process committed data, see docs
    struct Operation {
        OpType  opType;
        uint32  tokenId;
        address owner;
        uint112 amount;
    }

    // Total number of registered operations
    uint totalOperations;

    // List of operations by index
    mapping (uint64 => Operation) operations;

    // Total number of verified blocks
    // i.e. blocks[totalBlocksVerified] points at the latest verified block (block 0 is genesis)
    uint256 totalBlocksVerified;

    // Total number of committed blocks
    // i.e. blocks[totalBlocksCommitted] points at the latest committed block
    uint256 totalBlocksCommitted;

    // Block data (once per block)
    struct Block {

        // Hash of committment to public data for the block circuit
        bytes32 dataCommitment;

        // New root hash
        bytes32 stateRoot;

        // ETH block number at which this block was committed
        uint32  committedAtBlock;

        // ETH block number at which this block was verified
        uint32  verifiedAtBlock;

        // Validator (aka block producer)
        address validator;

        // Index of the first operation to process for this block
        uint64  operationStartId;

        // Total number of operations to process for this block
        uint64  totalOperations;
    }

    // List of blocks by Franklin blockId
    mapping (uint32 => Block) public blocks;

    // Flag indicating that exodus (mass exit) mode is triggered
    // Once it was raised, it can not be cleared again, and all users must exit
    bool exodusMode;

    // Flag indicating that a user has exited certain token balance (per owner and tokenId)
    mapping (address => mapping (uint32 => Balance)) public exited;


    constructor(bytes32 _genesisRoot, address _exitQueue, address _networkGovernance ) public {
        blocks[0].stateRoot = _genesisRoot;
        exitQueue = _exitQueue;
        networkGovernance = _networkGovernance; // for testing, use simple sender address
    }


}