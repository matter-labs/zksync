// SPDX-License-Identifier: MIT OR Apache-2.0
// solhint-disable max-states-count

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "./IERC20.sol";

import "./Governance.sol";
import "./Verifier.sol";
import "./Operations.sol";
import "./NFTFactory.sol";
import "./AdditionalZkSync.sol";

/// @title zkSync storage contract
/// @author Matter Labs
contract Storage {
    /// @dev Flag indicates that upgrade preparation status is active
    /// @dev Will store false in case of not active upgrade mode
    bool internal upgradePreparationActive;

    /// @dev Upgrade preparation activation timestamp (as seconds since unix epoch)
    /// @dev Will be equal to zero in case of not active upgrade mode
    uint256 internal upgradePreparationActivationTime;

    /// @dev Verifier contract. Used to verify block proof and exit proof
    Verifier internal verifier;

    /// @dev Governance contract. Contains the governor (the owner) of whole system, validators list, possible tokens list
    Governance internal governance;

    uint8 internal constant FILLED_GAS_RESERVE_VALUE = 0xff; // we use it to set gas revert value so slot will not be emptied with 0 balance
    struct PendingBalance {
        uint128 balanceToWithdraw;
        uint8 gasReserveValue; // gives user opportunity to fill storage slot with nonzero value
    }

    /// @dev Root-chain balances (per owner and token id, see packAddressAndTokenId) to withdraw
    mapping(bytes22 => PendingBalance) internal pendingBalances;

    // @dev Pending withdrawals are not used in this version
    struct PendingWithdrawalDEPRECATED {
        address to;
        uint16 tokenId;
    }
    mapping(uint32 => PendingWithdrawalDEPRECATED) internal pendingWithdrawalsDEPRECATED;
    uint32 internal firstPendingWithdrawalIndexDEPRECATED;
    uint32 internal numberOfPendingWithdrawalsDEPRECATED;

    /// @dev Total number of executed blocks i.e. blocks[totalBlocksExecuted] points at the latest executed block (block 0 is genesis)
    uint32 public totalBlocksExecuted;

    /// @notice Total number of committed blocks i.e. blocks[totalBlocksCommitted] points at the latest committed block
    uint32 public totalBlocksCommitted;

    /// @Old rollup block stored data - not used in current version
    /// @member validator Block producer
    /// @member committedAtBlock ETH block number at which this block was committed
    /// @member cumulativeOnchainOperations Total number of operations in this and all previous blocks
    /// @member priorityOperations Total number of priority operations for this block
    /// @member commitment Hash of the block circuit commitment
    /// @member stateRoot New tree root hash
    ///
    /// Consider memory alignment when changing field order: https://solidity.readthedocs.io/en/v0.4.21/miscellaneous.html
    struct BlockDEPRECATED {
        uint32 committedAtBlock;
        uint64 priorityOperations;
        uint32 chunks;
        bytes32 withdrawalsDataHash; // can be restricted to 16 bytes to reduce number of required storage slots
        bytes32 commitment;
        bytes32 stateRoot;
    }
    mapping(uint32 => BlockDEPRECATED) internal blocksDEPRECATED;

    /// @dev Flag indicates that a user has exited in the exodus mode certain token balance (per account id and tokenId)
    mapping(uint32 => mapping(uint32 => bool)) internal performedExodus;

    /// @dev Flag indicates that exodus (mass exit) mode is triggered
    /// @dev Once it was raised, it can not be cleared again, and all users must exit
    bool public exodusMode;

    /// @dev User authenticated fact hashes for some nonce.
    mapping(address => mapping(uint32 => bytes32)) public authFacts;

    /// @notice Old Priority Operation container
    /// @member opType Priority operation type
    /// @member pubData Priority operation public data
    /// @member expirationBlock Expiration block number (ETH block) for this request (must be satisfied before)
    struct PriorityOperationDEPRECATED {
        Operations.OpType opType;
        bytes pubData;
        uint256 expirationBlock;
    }

    /// @dev Priority Requests mapping (request id - operation)
    /// @dev Contains op type, pubdata and expiration block of unsatisfied requests.
    /// @dev Numbers are in order of requests receiving
    mapping(uint64 => PriorityOperationDEPRECATED) internal priorityRequestsDEPRECATED;

    /// @dev First open priority request id
    uint64 public firstPriorityRequestId;

    /// @dev Total number of requests
    uint64 public totalOpenPriorityRequests;

    /// @dev Total number of committed requests.
    /// @dev Used in checks: if the request matches the operation on Rollup contract and if provided number of requests is not too big
    uint64 internal totalCommittedPriorityRequests;

    /// @notice Packs address and token id into single word to use as a key in balances mapping
    function packAddressAndTokenId(address _address, uint16 _tokenId) internal pure returns (bytes22) {
        return bytes22((uint176(_address) | (uint176(_tokenId) << 160)));
    }

    /// @Rollup block stored data
    /// @member blockNumber Rollup block number
    /// @member priorityOperations Number of priority operations processed
    /// @member pendingOnchainOperationsHash Hash of all operations that must be processed after verify
    /// @member timestamp Rollup block timestamp, have the same format as Ethereum block constant
    /// @member stateHash Root hash of the rollup state
    /// @member commitment Verified input for the zkSync circuit
    struct StoredBlockInfo {
        uint32 blockNumber;
        uint64 priorityOperations;
        bytes32 pendingOnchainOperationsHash;
        uint256 timestamp;
        bytes32 stateHash;
        bytes32 commitment;
    }

    /// @notice Returns the keccak hash of the ABI-encoded StoredBlockInfo
    function hashStoredBlockInfo(StoredBlockInfo memory _storedBlockInfo) internal pure returns (bytes32) {
        return keccak256(abi.encode(_storedBlockInfo));
    }

    /// @dev Stored hashed StoredBlockInfo for some block number
    mapping(uint32 => bytes32) public storedBlockHashes;

    /// @dev Total blocks proven.
    uint32 internal totalBlocksProven;

    /// @notice Priority Operation container
    /// @member hashedPubData Hashed priority operation public data
    /// @member expirationBlock Expiration block number (ETH block) for this request (must be satisfied before)
    /// @member opType Priority operation type
    struct PriorityOperation {
        bytes20 hashedPubData;
        uint64 expirationBlock;
        Operations.OpType opType;
    }

    /// @dev Priority Requests mapping (request id - operation)
    /// @dev Contains op type, pubdata and expiration block of unsatisfied requests.
    /// @dev Numbers are in order of requests receiving
    mapping(uint64 => PriorityOperation) internal priorityRequests;

    /// @dev Timer for authFacts entry reset (address, nonce -> timer).
    /// @dev Used when user wants to reset `authFacts` for some nonce.
    mapping(address => mapping(uint32 => uint256)) internal authFactsResetTimer;

    mapping(uint32 => address) internal withdrawnNFTs;

    mapping(uint32 => Operations.WithdrawNFT) internal pendingWithdrawnNFTs;

    AdditionalZkSync internal additionalZkSync;

    /// @dev Upgrade notice period, possibly shorten by the security council
    uint256 internal approvedUpgradeNoticePeriod;

    /// @dev Upgrade start timestamp (as seconds since unix epoch)
    /// @dev Will be equal to zero in case of not active upgrade mode
    uint256 internal upgradeStartTimestamp;

    /// @dev Stores boolean flags which means the confirmations of the upgrade for each member of security council
    /// @dev Will store zeroes in case of not active upgrade mode
    mapping(uint256 => bool) internal securityCouncilApproves;
    uint256 internal numberOfApprovalsFromSecurityCouncil;
}
