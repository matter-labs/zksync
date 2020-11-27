pragma solidity ^0.5.8;

import "generated/ZkSyncTest.sol";

contract ZKSyncSignatureUnitTest is ZkSyncTest {
    bytes32 internal constant ADDITIONAL_HASH = 0x0;

    function changePubkeySignatureCheck(
        bytes calldata _signature,
        bytes20 _newPkHash,
        uint32 _nonce,
        address _ethAddress,
        uint24 _accountId
    ) external pure returns (bool) {
        return
            verifyChangePubkeySignature(
                abi.encodePacked(_signature, ADDITIONAL_HASH),
                _newPkHash,
                _nonce,
                _ethAddress,
                _accountId
            );
    }

    function testRecoverAddressFromEthSignature(bytes calldata _signature, bytes calldata _message)
        external
        pure
        returns (address)
    {
        return Utils.recoverAddressFromEthSignature(abi.encodePacked(_signature, ADDITIONAL_HASH), _message);
    }
}
