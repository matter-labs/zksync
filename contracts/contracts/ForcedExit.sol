// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "./Utils.sol";
import "./Ownable.sol"; 

contract SyncForcedExit is Ownable {
    // This is the role of the zkSync server
    // that will be able to withdraw the funds
    address public deputy;

    bool public enabled = true;

    constructor() Ownable(msg.sender) {}

    event FundsReceived(
        uint256 _amount
    );

    function setDeputy(address _newDeputy) external {
        requireMaster(msg.sender);

        deputy = _newDeputy;
    }

    function requireMasterOrDeputy(address _address) internal view {
        require(_address == deputy || _address == getMaster(), "only by deputy or master");
    }

    function withdrawFunds(address payable _to) external {
        requireMasterOrDeputy(msg.sender);

        (bool success, ) = _to.call{value: address(this).balance}("");
        require(success, "d"); // ETH withdraw failed
    }

    function disable() external {
        requireMaster(msg.sender);

        enabled = false;
    }

    function enable() external {
        requireMaster(msg.sender);

        enabled = true;
    }

    receive() external payable {
        require(enabled, "Contract is disabled");
        
        emit FundsReceived(msg.value);
    }
}
