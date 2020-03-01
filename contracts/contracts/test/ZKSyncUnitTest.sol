pragma solidity 0.5.16;

import "../generated/FranklinTest.sol";


contract ZKSyncUnitTest is FranklinTest {

    constructor(
        address _governanceAddress,
        address _verifierAddress,
        address _genesisAccAddress,
        bytes32 _genesisRoot
    ) FranklinTest(_governanceAddress, _verifierAddress, _genesisAccAddress, _genesisRoot) public{}

    function changePubkeySignatureCheck(bytes calldata _signature, bytes calldata _newPkHash, uint32 _nonce, address _ethAddress) external pure returns (bool) {
        return verifyChangePubkeySignature(_signature, _newPkHash, _nonce, _ethAddress);
    }

    function setBalanceToWithdraw(address _owner, uint16 _token, uint128 _amount) external {
        balancesToWithdraw[_owner][_token] = _amount;
    }

    function () payable external{}

    function addPendingWithdrawal(address _to, uint16 _tokenId, uint128 _amount) external {
        storeWithdrawalAsPending(_to, _tokenId, _amount);
    }

    function testProcessNextOperation(
        uint256 _pubdataOffset,
        bytes calldata _publicData,
        bytes calldata _currentEthWitness,
        uint256 _expectedBytesProcessed
    ) external {
        require(processNextOperation(_pubdataOffset, _publicData, _currentEthWitness) == _expectedBytesProcessed, "bytes processed incorrect");
    }

}
