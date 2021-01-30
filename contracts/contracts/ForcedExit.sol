// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "./Utils.sol";
import "./Ownable.sol"; 
import "./ReentrancyGuard.sol";

contract ForcedExit is Ownable, ReentrancyGuard {
    // This is the role of the zkSync server
    // that will be able to withdraw the funds
    address payable public receiver;

    bool public enabled = true;

    constructor() Ownable(msg.sender) {
        initializeReentrancyGuard();
    }

    event FundsReceived(
        uint256 _amount
    );

    function setReceiver(address payable _newReceiver) external {
        requireMaster(msg.sender);

        receiver = _newReceiver;
    }

    function disable() external {
        requireMaster(msg.sender);

        enabled = false;
    }

    function enable() external {
        requireMaster(msg.sender);

        enabled = true;
    }

    // Withdraw funds that failed to reach zkSync due to out-of-gas 
    function withdrawPendingFunds(address payable _to, uint128 amount) external nonReentrant {
        requireMaster(msg.sender);

        uint256 balance = address(this).balance;

        require(amount <= balance, "The balance is lower than the amount");
        
        (bool success, ) = _to.call{value: amount}("");
        require(success, "d"); // ETH withdraw failed
    }

    receive() external payable nonReentrant {
        require(enabled, "Contract is disabled");

        emit FundsReceived(msg.value);

        (bool success, ) = receiver.call{value: msg.value}("");
        require(success, "d"); // ETH withdraw failed
    }
}
