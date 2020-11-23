// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "../ZkSync.sol";

contract ZKSyncSignatureUnitTest is ZkSync {
        function changePubkeySignatureCheck(bytes calldata _signature, bytes20 _newPkHash, uint32 _nonce, address _ethAddress, uint24 _accountId) external pure returns (bool) {
            Operations.ChangePubKey memory changePk;
            changePk.owner = _ethAddress;
            changePk.nonce = _nonce;
            changePk.pubKeyHash = _newPkHash;
            changePk.accountId = _accountId;
            bytes memory witness = abi.encodePacked(byte(0x01), _signature, bytes32(0));
            return verifyChangePubkeyECRECOVER(witness, changePk);
        }

        function testRecoverAddressFromEthSignature(bytes calldata _signature, bytes32 _messageHash) external pure returns (address) {
            return Utils.recoverAddressFromEthSignature(_signature, _messageHash);
        }
}
