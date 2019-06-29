pragma solidity ^0.5.8;


contract Franklin {

    address creator;
    address public exitQueue;
    address public networkGovernance;

    uint totalTokens;
    mapping (uint32 => address) public tokens;

    struct Balance {
        uint112 balance;
        uint112 amountLocked;
        uint32  lockedUntilBlock;
    }

    mapping (address => mapping (uint32 => Balance)) public balances;

    uint256 totalBlocksCommitted;
    uint256 totalBlocksVerified;

    struct Block {
        bytes32 dataCommitment;
        bytes32 stateRoot;
        uint32  createdAtBlock;
        uint32  verifiedAtBlock;
        address validator;
    }

    // Key is block number
    mapping (uint32 => Block) public blocks;

    bool exodusMode;
    mapping (bytes32 => bool) exited;

    constructor(bytes32 _genesisRoot, address _exitQueue, address _networkGovernance) public {
        creator = msg.sender;
        blocks[0].stateRoot = _genesisRoot;
        exitQueue = _exitQueue;
        networkGovernance = _networkGovernance;
    }
}