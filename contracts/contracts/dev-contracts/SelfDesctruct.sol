// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

contract SelfDesctruct {

    function kill(address payable to) external {
        selfdestruct(to);
    }

}
