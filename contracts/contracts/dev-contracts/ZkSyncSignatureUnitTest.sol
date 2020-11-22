// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "../ZkSync.sol";

contract ZKSyncSignatureUnitTest is ZkSync {
    // TODO:
    //    function changePubkeySignatureCheck(bytes calldata _signature, bytes20 _newPkHash, uint32 _nonce, address _ethAddress, uint24 _accountId) external pure returns (bool) {
    //        return verifyChangePubkeySignature(_signature, _newPkHash, _nonce, _ethAddress, _accountId);
    //    }
    //
    //    function testRecoverAddressFromEthSignature(bytes calldata _signature, bytes calldata _message) external pure returns (address) {
    //        return Utils.recoverAddressFromEthSignature(_signature, _message);
    //    }
}
