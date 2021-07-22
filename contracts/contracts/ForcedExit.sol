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

    constructor(address _master, address _receiver) Ownable(_master) {
        initializeReentrancyGuard();

        // The master is the default receiver
        receiver = payable(_receiver);
    }

    event FundsReceived(uint256 _amount);

    function setReceiver(address payable _newReceiver) external {
        requireMaster(msg.sender);

        receiver = _newReceiver;
    }

    function withdrawPendingFunds(address payable _to) external nonReentrant {
        require(
            msg.sender == receiver || msg.sender == getMaster(),
            "Only the receiver or master can withdraw funds from the smart contract"
        );

        uint256 balance = address(this).balance;

        (bool success, ) = _to.call{value: balance}("");
        require(success, "ETH withdraw failed");
    }

    // We have to use fallback instead of `receive` since the ethabi
    // library can't decode the receive function:
    // https://github.com/rust-ethereum/ethabi/issues/185
    fallback() external payable {
        emit FundsReceived(msg.value);
    }
}
