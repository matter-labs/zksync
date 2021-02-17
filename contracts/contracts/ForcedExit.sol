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

    constructor(address _master) Ownable(_master) {
        initializeReentrancyGuard();

        // The master is the default receiver
        receiver = payable(_master);
    }

    event FundsReceived(uint256 _amount);

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
    // We don't require the contract to be enabled to call this function since
    // only the master can use it.
    function withdrawPendingFunds(address payable _to, uint128 amount) external nonReentrant {
        requireMaster(msg.sender);

        uint256 balance = address(this).balance;

        require(amount <= balance, "The balance is lower than the amount");

        (bool success, ) = _to.call{value: amount}("");
        require(success, "ETH withdraw failed");
    }

    // We have to use fallback instead of `receive` since the ethabi
    // library can't decode the receive function:
    // https://github.com/rust-ethereum/ethabi/issues/185
    fallback() external payable nonReentrant {
        require(enabled, "Contract is disabled");
        require(receiver != address(0), "Receiver must be non-zero");

        (bool success, ) = receiver.call{value: msg.value}("");
        require(success, "ETH withdraw failed");

        emit FundsReceived(msg.value);
    }
}
