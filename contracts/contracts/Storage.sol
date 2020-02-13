pragma solidity 0.5.16;

import "../node_modules/openzeppelin-solidity/contracts/token/ERC20/IERC20.sol";

import "./Governance.sol";
import "./Verifier.sol";
import "./PriorityQueue.sol";


/// @title zkSync storage contract
/// @author Matter Labs
contract Storage {

    /// @notice Verifier contract. Used to verify block proof and exit proof
    Verifier internal verifier;

    /// @notice Governance contract. Contains the governor (the owner) of whole system, validators list, possible tokens list
    Governance internal governance;

    /// @notice Priority Queue contract. Contains priority requests list
    PriorityQueue internal priorityQueue;

    /// @notice Root-chain balances (per owner and token id) to withdraw
    mapping(address => mapping(uint16 => uint128)) public balancesToWithdraw;

    /// @notice verified withdrawal pending to be executed.
    struct PendingWithdrawal {
        address to;
        uint16 tokenId;
    }
    
    /// @notice Verified but not executed withdrawals for addresses stored in here
    mapping(uint32 => PendingWithdrawal) public pendingWithdrawals;
    uint32 public firstPendingWithdrawalIndex;
    uint32 public numberOfPendingWithdrawals;

    /// @notice Total number of verified blocks i.e. blocks[totalBlocksVerified] points at the latest verified block (block 0 is genesis)
    uint32 public totalBlocksVerified;

    /// @notice Total number of committed blocks i.e. blocks[totalBlocksCommitted] points at the latest committed block
    uint32 public totalBlocksCommitted;

    /// @notice Rollup block data (once per block)
    /// @member validator Block producer
    /// @member committedAtBlock ETH block number at which this block was committed
    /// @member operationStartId Index of the first operation to process for this block
    /// @member onchainOperations Total number of operations to process for this block
    /// @member priorityOperations Total number of priority operations for this block
    /// @member commitment Hash of the block circuit commitment
    /// @member stateRoot New tree root hash
    struct Block {
        address validator;
        uint32 committedAtBlock;
        uint64 operationStartId;
        uint64 onchainOperations;
        uint64 priorityOperations;
        bytes32 commitment;
        bytes32 stateRoot;
    }

    /// @notice Blocks by Franklin block id
    mapping(uint32 => Block) public blocks;

    /// @notice Onchain operations - operations processed inside rollup blocks
    /// @member opType Onchain operation type
    /// @member amount Amount used in the operation
    /// @member pubData Operation pubdata
    struct OnchainOperation {
        Operations.OpType opType;
        bytes pubData;
    }

    /// @notice Total number of registered onchain operations
    uint64 public totalOnchainOps;

    /// @notice Onchain operations by index
    mapping(uint64 => OnchainOperation) public onchainOps;

    /// @notice Flag indicates that a user has exited certain token balance (per owner and tokenId)
    mapping(address => mapping(uint16 => bool)) public exited;

    /// @notice Flag indicates that exodus (mass exit) mode is triggered
    /// @notice Once it was raised, it can not be cleared again, and all users must exit
    bool public exodusMode;

    /// @notice User authenticated facts for some nonce.
    mapping(address => mapping(uint32 => bytes)) public authFacts;

}
