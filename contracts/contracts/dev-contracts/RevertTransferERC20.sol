// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

import "./TestnetERC20Token.sol";

/// @title RevertTransferERC20Token - A ERC20 token contract which can revert transfers depending on a flag
/// @dev Used for testing failed ERC-20 withdrawals from the zkSync smart contract
contract RevertTransferERC20 is TestnetERC20Token {
    bool public revertTransfer;

    constructor(
        string memory name,
        string memory symbol,
        uint8 decimals
    ) TestnetERC20Token(name, symbol, decimals) {
        revertTransfer = false;
    }

    function setRevertTransfer(bool newValue) public {
        revertTransfer = newValue;
    }

    function transfer(address recipient, uint256 amount) public virtual override returns (bool) {
        // Assert is used here to also simulate the out-of-gas error, since failed asserion
        // consumes up all the remaining gas
        assert(!revertTransfer);

        _transfer(_msgSender(), recipient, amount);
        return true;
    }
}
