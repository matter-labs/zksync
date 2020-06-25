pragma solidity ^0.5.0;

import "./Upgradeable.sol";
import "./Operations.sol";


/// @title zkSync events
/// @author Matter Labs
interface Events {

    /// @notice Event emitted when a block is committed
    event BlockCommit(uint32 indexed blockNumber);

    /// @notice Event emitted when a block is verified
    event BlockVerification(uint32 indexed blockNumber);

    /// @notice Event emitted when user send a transaction to withdraw her funds from onchain balance
    event OnchainWithdrawal(
        address indexed owner,
        uint16 indexed tokenId,
        uint128 amount
    );

    /// @notice Event emitted when user send a transaction to deposit her funds
    event OnchainDeposit(
        address indexed sender,
        uint16 indexed tokenId,
        uint128 amount,
        address indexed owner
    );

    /// @notice Event emitted when user sends a authentication fact (e.g. pub-key hash)
    event FactAuth(
        address indexed sender,
        uint32 nonce,
        bytes fact
    );

    /// @notice Event emitted when blocks are reverted
    event BlocksRevert(
        uint32 totalBlocksVerified,
        uint32 totalBlocksCommitted
    );

    /// @notice Exodus mode entered event
    event ExodusMode();

    /// @notice New priority request event. Emitted when a request is placed into mapping
    event NewPriorityRequest(
        address sender,
        uint64 serialId,
        Operations.OpType opType,
        bytes pubData,
        uint256 expirationBlock
    );

    /// @notice Deposit committed event.
    event DepositCommit(
        uint32 indexed zkSyncBlockId,
        uint32 indexed accountId,
        address owner,
        uint16 indexed tokenId,
        uint128 amount
    );

    /// @notice Full exit committed event.
    event FullExitCommit(
        uint32 indexed zkSyncBlockId,
        uint32 indexed accountId,
        address owner,
        uint16 indexed tokenId,
        uint128 amount
    );

    /// @notice Pending withdrawals index range that were added in the verifyBlock operation.
    /// NOTE: processed indexes in the queue map are [queueStartIndex, queueEndIndex)
    event PendingWithdrawalsAdd(
        uint32 queueStartIndex,
        uint32 queueEndIndex
    );

    /// @notice Pending withdrawals index range that were executed in the completeWithdrawals operation.
    /// NOTE: processed indexes in the queue map are [queueStartIndex, queueEndIndex)
    event PendingWithdrawalsComplete(
        uint32 queueStartIndex,
        uint32 queueEndIndex
    );
}

/// @title Upgrade events
/// @author Matter Labs
interface UpgradeEvents {

    /// @notice Event emitted when new upgradeable contract is added to upgrade gatekeeper's list of managed contracts
    event NewUpgradable(
        uint indexed versionId,
        address indexed upgradeable
    );

    /// @notice Upgrade mode enter event
    event NoticePeriodStart(
        uint indexed versionId,
        address[] newTargets,
        uint noticePeriod // notice period (in seconds)
    );

    /// @notice Upgrade mode cancel event
    event UpgradeCancel(
        uint indexed versionId
    );

    /// @notice Upgrade mode preparation status event
    event PreparationStart(
        uint indexed versionId
    );

    /// @notice Upgrade mode complete event
    event UpgradeComplete(
        uint indexed versionId,
        address[] newTargets
    );

}
