pragma solidity 0.5.16;

import "../Franklin.sol";


contract ZKSyncUnitTest is Franklin {

    constructor() Franklin(address(0), address(0), address(0), bytes32(0)) public{}

    function changePubkeySignatureCheck(bytes calldata _signature, bytes calldata _newPkHash, uint32 _nonce, address _ethAddress) external pure returns (bool) {
        return verifyChangePubkeySignature(_signature, _newPkHash, _nonce, _ethAddress);
    }

}

