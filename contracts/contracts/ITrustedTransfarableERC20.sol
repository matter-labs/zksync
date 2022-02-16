/// @dev Interface of the ERC20 standard as defined in the EIP.
/// 1. Implements only `transfer` and `transferFrom` methods
/// 2. These methods return a boolean value in case of a non-revert call
/// NOTE: It is expected that if the function returns true, then the user's balance has
/// changed exactly by `amount` according to the ERC20 standard.
/// Note: Used to perform transfers for tokens that explicitly return a boolean value
/// (if the token returns any other data or does not return at all, then the function call will be reverted)
interface ITrustedTransfarableERC20 {
    /**
     * @dev Moves `amount` tokens from the caller's account to `recipient`.
     *
     * Returns a boolean value indicating whether the operation succeeded.
     *
     * Emits a {Transfer} event.
     */
    function transfer(address recipient, uint256 amount) external returns (bool);

    /**
     * @dev Moves `amount` tokens from `sender` to `recipient` using the
     * allowance mechanism. `amount` is then deducted from the caller's
     * allowance.
     *
     * Returns a boolean value indicating whether the operation succeeded.
     *
     * Emits a {Transfer} event.
     */
    function transferFrom(
        address sender,
        address recipient,
        uint256 amount
    ) external returns (bool);
}
