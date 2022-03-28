// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

contract DummyERC20BytesTransferReturnValue {
    bytes returnValue;

    constructor(bytes memory _returnValue) {
        returnValue = _returnValue;
    }

    function transfer(address _recipient, uint256 _amount) external view returns (bytes memory) {
        // Hack to prevent Solidity warnings
        _recipient;
        _amount;

        return returnValue;
    }
}
