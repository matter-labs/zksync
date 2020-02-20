pragma solidity 0.5.16;


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
        uint256 fee,
        address indexed owner
    );

    event FactAuth(
        address sender,
        uint32 nonce,
        bytes fact
    );

    /// @notice Event emitted when blocks are reverted
    event BlocksReverted(
        uint32 indexed totalBlocksVerified,
        uint32 indexed totalBlocksCommitted
    );

    /// @notice Exodus mode entered event
    event ExodusMode();

    /// @notice New priority request event. Emitted when a request is placed into mapping
    event NewPriorityRequest(
        address sender,
        uint64 serialId,
        uint8 opType,
        bytes pubData,
        uint256 expirationBlock,
        uint256 fee
    );
}

/// @title Upgrade mode events
/// @author Matter Labs
contract UpgradeModeEvents {

    /// @notice Upgrade mode enter event
    event UpgradeModeActivated(
        uint64 version
    );

    /// @notice Upgrade mode cancel event
    event UpgradeCanceled(
        uint64 version
    );

    /// @notice Upgrade mode closed status event
    event UpgradeModeClosedStatusActivated(
        uint64 version
    );

    /// @notice Upgrade mode complete event
    event UpgradeCompleted(
        uint64 version
    );

}
