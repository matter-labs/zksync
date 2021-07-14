// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

/// @title zkSync configuration constants
/// @author Matter Labs
contract Config {
    /// @dev ERC20 tokens and ETH withdrawals gas limit, used only for complete withdrawals
    uint256 internal constant WITHDRAWAL_GAS_LIMIT = 100000;

    /// @dev NFT withdrawals gas limit, used only for complete withdrawals
    uint256 internal constant WITHDRAWAL_NFT_GAS_LIMIT = 300000;

    /// @dev Bytes in one chunk
    uint8 internal constant CHUNK_BYTES = 10;

    /// @dev zkSync address length
    uint8 internal constant ADDRESS_BYTES = 20;

    uint8 internal constant PUBKEY_HASH_BYTES = 20;

    /// @dev Public key bytes length
    uint8 internal constant PUBKEY_BYTES = 32;

    /// @dev Ethereum signature r/s bytes length
    uint8 internal constant ETH_SIGN_RS_BYTES = 32;

    /// @dev Success flag bytes length
    uint8 internal constant SUCCESS_FLAG_BYTES = 1;

    /// @dev Max amount of tokens registered in the network (excluding ETH, which is hardcoded as tokenId = 0)
    uint16 internal constant MAX_AMOUNT_OF_REGISTERED_TOKENS = $(MAX_AMOUNT_OF_REGISTERED_TOKENS);

    /// @dev Max account id that could be registered in the network
    uint32 internal constant MAX_ACCOUNT_ID = $$((2**24) - 1);

    /// @dev Expected average period of block creation
    uint256 internal constant BLOCK_PERIOD = 15 seconds;

    /// @dev ETH blocks verification expectation
    /// @dev Blocks can be reverted if they are not verified for at least EXPECT_VERIFICATION_IN.
    /// @dev If set to 0 validator can revert blocks at any time.
    uint256 internal constant EXPECT_VERIFICATION_IN = 0 hours / BLOCK_PERIOD;

    uint256 internal constant NOOP_BYTES = 1 * CHUNK_BYTES;
    uint256 internal constant DEPOSIT_BYTES = 6 * CHUNK_BYTES;
    uint256 internal constant MINT_NFT_BYTES = 5 * CHUNK_BYTES;
    uint256 internal constant TRANSFER_TO_NEW_BYTES = 6 * CHUNK_BYTES;
    uint256 internal constant PARTIAL_EXIT_BYTES = 6 * CHUNK_BYTES;
    uint256 internal constant TRANSFER_BYTES = 2 * CHUNK_BYTES;
    uint256 internal constant FORCED_EXIT_BYTES = 6 * CHUNK_BYTES;
    uint256 internal constant WITHDRAW_NFT_BYTES = 10 * CHUNK_BYTES;

    /// @dev Full exit operation length
    uint256 internal constant FULL_EXIT_BYTES = 11 * CHUNK_BYTES;

    /// @dev ChangePubKey operation length
    uint256 internal constant CHANGE_PUBKEY_BYTES = 6 * CHUNK_BYTES;

    /// @dev Expiration delta for priority request to be satisfied (in seconds)
    /// @dev NOTE: Priority expiration should be > (EXPECT_VERIFICATION_IN * BLOCK_PERIOD)
    /// @dev otherwise incorrect block with priority op could not be reverted.
    uint256 internal constant PRIORITY_EXPIRATION_PERIOD = 3 days;

    /// @dev Expiration delta for priority request to be satisfied (in ETH blocks)
    uint256 internal constant PRIORITY_EXPIRATION =
        $(defined(PRIORITY_EXPIRATION) ? PRIORITY_EXPIRATION : PRIORITY_EXPIRATION_PERIOD / BLOCK_PERIOD);

    /// @dev Maximum number of priority request to clear during verifying the block
    /// @dev Cause deleting storage slots cost 5k gas per each slot it's unprofitable to clear too many slots
    /// @dev Value based on the assumption of ~750k gas cost of verifying and 5 used storage slots per PriorityOperation structure
    uint64 internal constant MAX_PRIORITY_REQUESTS_TO_DELETE_IN_VERIFY = 6;

    /// @dev Reserved time for users to send full exit priority operation in case of an upgrade (in seconds)
    uint256 internal constant MASS_FULL_EXIT_PERIOD = 9 days;

    /// @dev Reserved time for users to withdraw funds from full exit priority operation in case of an upgrade (in seconds)
    uint256 internal constant TIME_TO_WITHDRAW_FUNDS_FROM_FULL_EXIT = 2 days;

    /// @dev Notice period before activation preparation status of upgrade mode (in seconds)
    /// @dev NOTE: we must reserve for users enough time to send full exit operation, wait maximum time for processing this operation and withdraw funds from it.
    uint256 internal constant UPGRADE_NOTICE_PERIOD =
        $(
            defined(UPGRADE_NOTICE_PERIOD)
                ? UPGRADE_NOTICE_PERIOD
                : MASS_FULL_EXIT_PERIOD + PRIORITY_EXPIRATION_PERIOD + TIME_TO_WITHDRAW_FUNDS_FROM_FULL_EXIT
        );

    /// @dev Timestamp - seconds since unix epoch
    uint256 internal constant COMMIT_TIMESTAMP_NOT_OLDER = 24 hours;

    /// @dev Maximum available error between real commit block timestamp and analog used in the verifier (in seconds)
    /// @dev Must be used cause miner's `block.timestamp` value can differ on some small value (as we know - 15 seconds)
    uint256 internal constant COMMIT_TIMESTAMP_APPROXIMATION_DELTA = 15 minutes;

    /// @dev Bit mask to apply for verifier public input before verifying.
    uint256 internal constant INPUT_MASK = $$(~uint256(0) >> 3);

    /// @dev Auth fact reset timelock.
    uint256 internal constant AUTH_FACT_RESET_TIMELOCK = 1 days;

    /// @dev Max deposit of ERC20 token that is possible to deposit
    uint128 internal constant MAX_DEPOSIT_AMOUNT = $$((2**104) - 1);

    uint32 internal constant SPECIAL_ACCOUNT_ID = $$((2**24) - 1);
    address internal constant SPECIAL_ACCOUNT_ADDRESS = address(0xFFfFfFffFFfffFFfFFfFFFFFffFFFffffFfFFFfF);
    uint32 internal constant SPECIAL_NFT_TOKEN_ID = $$((2**31) - 2);

    uint32 internal constant MAX_FUNGIBLE_TOKEN_ID = $$((2**16) - 1);

    uint256 internal constant SECURITY_COUNCIL_MEMBERS_NUMBER = $$(SECURITY_COUNCIL_MEMBERS_NUMBER);
}
