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

    function processNextOperation(
        uint256 _pubdataOffset,
        bytes memory _publicData,
        bytes memory _currentEthWitness
    ) internal returns (uint256 _bytesProcessed) {
        uint8 opType = uint8(_publicData[_pubdataOffset]);

        if (opType == uint8(Operations.OpType.Noop)) return NOOP_BYTES;
        if (opType == uint8(Operations.OpType.TransferToNew)) return TRANSFER_TO_NEW_BYTES;
        if (opType == uint8(Operations.OpType.Transfer)) return TRANSFER_BYTES;
        if (opType == uint8(Operations.OpType.CloseAccount)) return CLOSE_ACCOUNT_BYTES;

        if (opType == uint8(Operations.OpType.Deposit)) {
            bytes memory pubData = Bytes.slice(_publicData, _pubdataOffset + 1, DEPOSIT_BYTES - 1);
            onchainOps[totalOnchainOps] = OnchainOperation(
                Operations.OpType.Deposit,
                pubData
            );
            verifyNextPriorityOperation(onchainOps[totalOnchainOps]);

            totalOnchainOps++;

            return DEPOSIT_BYTES;
        }

        if (opType == uint8(Operations.OpType.PartialExit)) {
            bytes memory pubData = Bytes.slice(_publicData, _pubdataOffset + 1, PARTIAL_EXIT_BYTES - 1);
            onchainOps[totalOnchainOps] = OnchainOperation(
                Operations.OpType.PartialExit,
                pubData
            );
            totalOnchainOps++;

            return PARTIAL_EXIT_BYTES;
        }

        if (opType == uint8(Operations.OpType.FullExit)) {
            bytes memory pubData = Bytes.slice(_publicData, _pubdataOffset + 1, FULL_EXIT_BYTES - 1);
            onchainOps[totalOnchainOps] = OnchainOperation(
                Operations.OpType.FullExit,
                pubData
            );

            verifyNextPriorityOperation(onchainOps[totalOnchainOps]);

            totalOnchainOps++;
            return FULL_EXIT_BYTES;
        }

        if (opType == uint8(Operations.OpType.ChangePubKey)) {
            Operations.ChangePubKey memory op = Operations.readChangePubKeyPubdata(_publicData, _pubdataOffset + 1);
            if (_currentEthWitness.length > 0) {
                bool valid = verifyChangePubkeySignature(_currentEthWitness, op.pubKeyHash, op.nonce, op.owner);
                require(valid, "fpp15"); // failed to verify change pubkey hash signature
            } else {
                bool valid = keccak256(authFacts[op.owner][op.nonce]) == keccak256(op.pubKeyHash);
                require(valid, "fpp16"); // new pub key hash is not authenticated properly
            }
            return CHANGE_PUBKEY_BYTES;
        }

        revert("fpp14"); // unsupported op
    }

    function testProcessNextOperation(
        uint256 _pubdataOffset,
        bytes calldata _publicData,
        bytes calldata _currentEthWitness,
        uint256 _expectedBytesProcessed
    ) external {
        require(processNextOperation(_pubdataOffset, _publicData, _currentEthWitness) == _expectedBytesProcessed, "bytes processed incorrect");
    }

    function testVerifyEthereumSignature(bytes calldata _signature, bytes calldata _message) external pure returns (address) {
        return verifyEthereumSignature(_signature, _message);
    }
}
