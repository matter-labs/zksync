pragma solidity ^0.5.0;

import "generated/ZkSyncTest.sol";


contract ZkSyncWithdrawalUnitTest is ZkSyncTest {

    function setBalanceToWithdraw(address _owner, uint16 _token, uint128 _amount) external {
        balancesToWithdraw[packAddressAndTokenId(_owner, _token)].balanceToWithdraw = _amount;
    }

    function receiveETH() payable external{}

    function addPendingWithdrawal(address _to, uint16 _tokenId, uint128 _amount) external {
        pendingWithdrawals[firstPendingWithdrawalIndex + numberOfPendingWithdrawals] = PendingWithdrawal(_to, _tokenId);
        numberOfPendingWithdrawals++;
        bytes22 packedBalanceKey = packAddressAndTokenId(_to, _tokenId);
        balancesToWithdraw[packedBalanceKey].balanceToWithdraw += _amount;
    }

}
