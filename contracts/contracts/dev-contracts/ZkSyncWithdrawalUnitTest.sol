// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "../ZkSync.sol";

contract ZkSyncWithdrawalUnitTest is ZkSync {
    function setBalanceToWithdraw(
        address _owner,
        uint16 _token,
        uint128 _amount
    ) external {
        pendingBalances[packAddressAndTokenId(_owner, _token)].balanceToWithdraw = _amount;
    }

    // solhint-disable-next-line no-empty-blocks
    function receiveETH() external payable {}

    function withdrawOrStoreExternal(
        uint16 _tokenId,
        address _recipient,
        uint128 _amount
    ) external {
        return withdrawOrStore(_tokenId, _recipient, _amount);
    }
}
