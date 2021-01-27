// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

/// @title RevertReceiveAccount - An account which reverts receiving funds depending on the flag
/// @dev Used for testing failed withdrawlas from the zkSync smart contract
contract RevertReceiveAccount {
    
    address public owner;

    bool public revertReceive;

    constructor() {
        owner = msg.sender;
        revertReceive = true;
    }

    function setRevertReceive(bool newValue) public {
        require(msg.sender == owner, "Only the owner can change if the request is reverted");

        revertReceive = newValue;
    }

    fallback() external payable {
        require(!revertReceive, "All the receiving transactions are reverted");
    }
}
