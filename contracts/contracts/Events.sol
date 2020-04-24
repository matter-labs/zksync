pragma solidity 0.5.16;

import "./Upgradeable.sol";


/// @title zkSync events
/// @author Matter Labs
contract Events {

    /// @notice Event emitted when a block is committed
    event BlockCommitted(uint32 indexed blockNumber);

    /// @notice Event emitted when a block is verified
    event BlockVerified(uint32 indexed blockNumber);

    /// @notice Event emitted when user send a transaction to withdraw her funds from onchain balance
    event OnchainWithdrawal(
        address indexed owner,
        uint16 tokenId,
        uint128 amount
    );

    /// @notice Event emitted when user send a transaction to deposit her funds
    event OnchainDeposit(
        address sender,
        uint16 tokenId,
        uint128 amount,
        address indexed owner
    );

    /// @notice Event emitted when user sends a authentication fact (e.g. pub-key hash)
    event FactAuth(
        address sender,
        uint32 nonce,
        bytes fact
    );

    /// @notice Event emitted when blocks are reverted
    event BlocksReverted(
        uint32 totalBlocksVerified,
        uint32 totalBlocksCommitted
    );

    /// @notice Exodus mode entered event
    event ExodusMode();

    /// @notice New priority request event. Emitted when a request is placed into mapping
    event NewPriorityRequest(
        address sender,
        uint64 serialId,
        uint8 opType,
        bytes pubData,
        uint256 expirationBlock
    );
}

/// @title Upgrade events
/// @author Matter Labs
contract UpgradeEvents {

    /// @notice Event emitted when new upgradeable contract is added to upgrade gatekeeper's list of managed contracts
    event UpgradeableAdd(
        Upgradeable upgradeable
    );

    /// @notice Upgrade mode enter event
    event NoticePeriodStart(
        address[] newTargets
    );

    /// @notice Upgrade mode cancel event
    event UpgradeCancel();

    /// @notice Upgrade mode preparation status event
    event PreparationStart();

    /// @notice Upgrade mode complete event
    event UpgradeComplete(
        Upgradeable upgradeable,
        address newTargetAddress
    );

}
