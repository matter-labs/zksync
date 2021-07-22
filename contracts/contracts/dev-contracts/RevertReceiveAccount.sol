// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

/// @title RevertReceiveAccount - An account which reverts receiving funds depending on the flag
/// @dev Used for testing failed withdrawals from the zkSync smart contract
contract RevertReceiveAccount {
    bool public revertReceive;

    constructor() {
        revertReceive = false;
    }

    function setRevertReceive(bool newValue) public {
        revertReceive = newValue;
    }

    receive() external payable {
        // Assert is used here to also simulate the out-of-gas error, since failed asserion
        // consumes up all the remaining gas
        assert(!revertReceive);
    }
}
