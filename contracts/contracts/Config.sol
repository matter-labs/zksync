pragma solidity ^0.5.0;


/// @title zkSync configuration constants
/// @author Matter Labs
contract Config {

    /// @notice Notice period before activation preparation status of upgrade mode (in seconds)
    uint constant UPGRADE_NOTICE_PERIOD = 1 days;

    /// @notice Period after the start of preparation upgrade when contract wouldn't register new priority operations (in seconds)
    uint constant UPGRADE_PREPARATION_LOCK_PERIOD = 1 days;

    /// @notice ERC20 token withdrawal gas limit, used only for complete withdrawals
    uint256 constant ERC20_WITHDRAWAL_GAS_LIMIT = 250000;

    /// @notice ETH token withdrawal gas limit, used only for complete withdrawals
    uint256 constant ETH_WITHDRAWAL_GAS_LIMIT = 10000;

    /// @notice zkSync address length
    uint8 constant ADDRESS_BYTES = 20;

    // TODO: check everywhere!
    uint8 constant PUBKEY_HASH_BYTES = 20;

    /// @notice Public key bytes length
    uint8 constant PUBKEY_BYTES = 32;

    /// @notice Ethereum signature r/s bytes length
    uint8 constant ETH_SIGN_RS_BYTES = 32;

    /// @notice Success flag bytes length
    uint8 constant SUCCESS_FLAG_BYTES = 1;

    /// @notice Max amount of tokens registered in the network (excluding ETH, which is hardcoded as tokenId = 0)
    uint16 constant MAX_AMOUNT_OF_REGISTERED_TOKENS = (2 ** 16) - 1;

    /// @notice Expected average period of block creation
    uint256 constant BLOCK_PERIOD = 15 seconds;

    /// @notice ETH blocks verification expectation
    uint256 constant EXPECT_VERIFICATION_IN = 2 days / BLOCK_PERIOD;

    uint256 constant NOOP_BYTES = 1 * 8;
    uint256 constant DEPOSIT_BYTES = 6 * 8;
    uint256 constant TRANSFER_TO_NEW_BYTES = 5 * 8;
    uint256 constant PARTIAL_EXIT_BYTES = 6 * 8;
    uint256 constant TRANSFER_BYTES = 2 * 8;

    /// @notice Full exit operation length
    uint256 constant FULL_EXIT_BYTES = 6 * 8;

    /// @notice OnchainWithdrawal data length
    uint256 constant ONCHAIN_WITHDRAWAL_BYTES = 1 + 20 + 2 + 16; // (uint8 addToPendingWithdrawalsQueue, address _to, uint16 _tokenId, uint128 _amount)

    /// @notice ChangePubKey operation length
    uint256 constant CHANGE_PUBKEY_BYTES = 6 * 8;

    /// @notice Expiration delta for priority request to be satisfied (in ETH blocks)
    /// NOTE: Priority expiration should be > EXPECT_VERIFICATION_IN, otherwise incorrect block with priority op could not be reverted.
    uint256 constant PRIORITY_EXPIRATION = 3 days / BLOCK_PERIOD;
}
