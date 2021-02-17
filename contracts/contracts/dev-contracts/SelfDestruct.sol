// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

contract SelfDestruct {
    function destroy(address payable to) external {
        selfdestruct(to);
    }

    // Need this to send some funds to the contract
    receive() external payable {}
}
