// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "../ZkSync.sol";

contract ZKSyncSignatureUnitTest is ZkSync {
    function changePubkeySignatureCheckECRECOVER(Operations.ChangePubKey memory _changePk, bytes calldata _witness)
        external
        pure
        returns (bool)
    {
        return verifyChangePubkeyECRECOVER(_witness, _changePk);
    }

    function changePubkeySignatureCheckCREATE2(Operations.ChangePubKey memory _changePk, bytes calldata _witness)
        external
        pure
        returns (bool)
    {
        return verifyChangePubkeyCREATE2(_witness, _changePk);
    }

    function testRecoverAddressFromEthSignature(bytes memory _signature, bytes32 _messageHash)
        external
        pure
        returns (address)
    {
        return Utils.recoverAddressFromEthSignature(_signature, _messageHash);
    }

    function changePubkeySignatureCheckEIP712(Operations.ChangePubKey memory _changePk, bytes calldata _witness)
        external
        pure
        returns (bool)
    {
        return verifyChangePubkeyEIP712(_witness, _changePk);
    }
}
