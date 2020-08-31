pragma solidity ^0.5.0;

import "generated/ZkSyncTest.sol";


contract ZKSyncSignatureUnitTest is ZkSyncTest {

    function changePubkeySignatureCheck(bytes calldata _signature, bytes20 _newPkHash, uint32 _nonce, address _ethAddress, uint24 _accountId) external pure returns (bool) {
        return verifyChangePubkeySignature(_signature, _newPkHash, _nonce, _ethAddress, _accountId);
    }

    function testRecoverAddressFromEthSignature(bytes calldata _signature, bytes calldata _message) external pure returns (address) {
        return Utils.recoverAddressFromEthSignature(_signature, _message);
    }

}
