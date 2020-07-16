pragma solidity ^0.5.0;

import "./SafeMath.sol";
import "./SafeMathUInt128.sol";
import "./Utils.sol";

import "./Storage.sol";
import "./Config.sol";
import "./Events.sol";

import "./Bytes.sol";
import "./Operations.sol";

/// @title BlockProcessor contract
/// This is a "library contract". ZkSync contract would use it only with delegatecalls.
/// This have 3 endpoints: `commitBlock`, `verifyBlock` and `revertBlocks`.
/// At the end of the contract, there are a few external functions, which allow testing some of the internal functional.
/// @author Matter Labs
contract BlockProcessor is Storage, Config, Events {
    using SafeMath for uint256;
    using SafeMathUInt128 for uint128;

    bytes32 public constant EMPTY_STRING_KECCAK = 0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470;

    /// @notice Commit block - collect onchain operations, create its commitment, emit BlockCommit event
    /// @param _blockNumber Block number
    /// @param _feeAccount Account to collect fees
    /// @param _blockTimestamp Timestamp to be used in the verifier
    /// @param _newBlockInfo New state of the block. (first element is the account tree root hash, rest of the array is reserved for the future)
    /// @param _publicData Operations pubdata
    /// @param _ethWitness Data passed to ethereum outside pubdata of the circuit.
    /// @param _ethWitnessSizes Amount of eth witness bytes for the corresponding operation.
    function commitBlock(
        uint32 _blockNumber,
        uint32 _feeAccount,
        uint64 _blockTimestamp,
        bytes32[] calldata _newBlockInfo,
        bytes calldata _publicData,
        bytes calldata _ethWitness,
        uint32[] calldata _ethWitnessSizes
    ) external {
        require(_blockNumber == totalBlocksCommitted + 1, "fck11"); // only commit next block
        governance.requireActiveValidator(msg.sender);

        require(blocks[totalBlocksCommitted].blockTimestamp < _blockTimestamp, "tms11"); // tms11 - _blockTimestamp smaller or equal than for the previous block
        require(now - COMMIT_TIMESTAMP_NOT_OLDER <= _blockTimestamp && _blockTimestamp <= (now + COMMIT_TIMESTAMP_APPROXIMATION_DELTA), "tms12"); // tms12 - _blockTimestamp is not valid
        require(_newBlockInfo.length == 1, "fck13"); // This version of the contract expects only account tree root hash

        // Unpack onchain operations and store them.
        // Get priority operations number for this block.
        uint64 prevTotalCommittedPriorityRequests = totalCommittedPriorityRequests;
        bytes32 withdrawalsDataHash = collectOnchainOps(_blockNumber, _publicData, _ethWitness, _ethWitnessSizes);
        uint64 nPriorityRequestProcessed = totalCommittedPriorityRequests - prevTotalCommittedPriorityRequests;

        createCommittedBlock(_blockNumber, _feeAccount, _blockTimestamp, _newBlockInfo[0], _publicData, withdrawalsDataHash, nPriorityRequestProcessed);
        totalBlocksCommitted++;

        emit BlockCommit(_blockNumber);
    }

    /// @notice Block verification.
    /// @notice Verify proof -> process onchain withdrawals (accrue balances from withdrawals) -> remove priority requests
    /// @param _blockNumber Block number
    /// @param _proof Block proof
    /// @param _withdrawalsData Block withdrawals data
    function verifyBlock(uint32 _blockNumber, uint256[] calldata _proof, bytes calldata _withdrawalsData)
        external
    {
        require(_blockNumber == totalBlocksVerified + 1, "fvk11"); // only verify next block
        governance.requireActiveValidator(msg.sender);

        require(verifier.verifyBlockProof(_proof, blocks[_blockNumber].commitment, blocks[_blockNumber].chunks), "fvk13"); // proof verification failed

        processOnchainWithdrawals(_withdrawalsData, blocks[_blockNumber].withdrawalsDataHash);

        deleteRequests(
            blocks[_blockNumber].priorityOperations
        );

        totalBlocksVerified += 1;

        emit BlockVerification(_blockNumber);
    }


    /// @notice Reverts unverified blocks
    /// @param _maxBlocksToRevert the maximum number blocks that will be reverted (use if can't revert all blocks because of gas limit).
    function revertBlocks(uint32 _maxBlocksToRevert) external {
        require(isBlockCommitmentExpired(), "rbs11"); // trying to revert non-expired blocks.
        governance.requireActiveValidator(msg.sender);

        uint32 blocksCommited = totalBlocksCommitted;
        uint32 blocksToRevert = Utils.minU32(_maxBlocksToRevert, blocksCommited - totalBlocksVerified);
        uint64 revertedPriorityRequests = 0;

        for (uint32 i = totalBlocksCommitted - blocksToRevert + 1; i <= blocksCommited; i++) {
            Block memory revertedBlock = blocks[i];
            require(revertedBlock.committedAtBlock > 0, "frk11"); // block not found

            revertedPriorityRequests += revertedBlock.priorityOperations;

            delete blocks[i];
        }

        blocksCommited -= blocksToRevert;
        totalBlocksCommitted -= blocksToRevert;
        totalCommittedPriorityRequests -= revertedPriorityRequests;

        emit BlocksRevert(totalBlocksVerified, blocksCommited);
    }

    /// @notice Store committed block structure to the storage.
    /// @param _nCommittedPriorityRequests - number of priority requests in block
    function createCommittedBlock(
        uint32 _blockNumber,
        uint32 _feeAccount,
        uint64 _blockTimestamp,
        bytes32 _newRoot,
        bytes memory _publicData,
        bytes32 _withdrawalDataHash,
        uint64 _nCommittedPriorityRequests
    ) internal {
        require(_publicData.length % CHUNK_BYTES == 0, "cbb10"); // Public data size is not multiple of CHUNK_BYTES

        uint32 blockChunks = uint32(_publicData.length / CHUNK_BYTES);
        require(verifier.isBlockSizeSupported(blockChunks), "ccb11");

        // Create block commitment for verification proof
        bytes32 commitment = createBlockCommitment(
            _blockNumber,
            _feeAccount,
            _blockTimestamp,
            blocks[_blockNumber - 1].stateRoot,
            _newRoot,
            _publicData
        );

        blocks[_blockNumber] = Block(
            uint32(block.number), // committed at
            _nCommittedPriorityRequests, // number of priority onchain ops in block
            blockChunks,
            _withdrawalDataHash, // hash of onchain withdrawals data (will be used during checking block withdrawal data in verifyBlock function)
            commitment, // blocks' commitment
            _newRoot, // new root
            _blockTimestamp // block timestamp to be used in the verifier
        );
    }

    function emitDepositCommitEvent(uint32 _blockNumber, Operations.Deposit memory depositData) internal {
        emit DepositCommit(_blockNumber, depositData.accountId, depositData.owner, depositData.tokenId, depositData.amount);
    }

    function emitFullExitCommitEvent(uint32 _blockNumber, Operations.FullExit memory fullExitData) internal {
        emit FullExitCommit(_blockNumber, fullExitData.accountId, fullExitData.owner, fullExitData.tokenId, fullExitData.amount);
    }

    /// @notice Gets operations packed in bytes array. Unpacks it and stores onchain operations.
    /// @param _blockNumber Franklin block number
    /// @param _publicData Operations packed in bytes array
    /// @param _ethWitness Eth witness that was posted with commit
    /// @param _ethWitnessSizes Amount of eth witness bytes for the corresponding operation.
    /// Priority operations must be committed in the same order as they are in the priority queue.
    function collectOnchainOps(uint32 _blockNumber, bytes memory _publicData, bytes memory _ethWitness, uint32[] memory _ethWitnessSizes)
        internal returns (bytes32 withdrawalsDataHash) {
        require(_publicData.length % CHUNK_BYTES == 0, "fcs11"); // pubdata length must be a multiple of CHUNK_BYTES

        uint64 currentPriorityRequestId = firstPriorityRequestId + totalCommittedPriorityRequests;

        uint256 pubDataPtr = 0;
        uint256 pubDataStartPtr = 0;
        uint256 pubDataEndPtr = 0;

        assembly { pubDataStartPtr := add(_publicData, 0x20) }
        pubDataPtr = pubDataStartPtr;
        pubDataEndPtr = pubDataStartPtr + _publicData.length;

        uint64 ethWitnessOffset = 0;
        uint16 processedOperationsRequiringEthWitness = 0;

        withdrawalsDataHash = EMPTY_STRING_KECCAK;

        while (pubDataPtr < pubDataEndPtr) {
            Operations.OpType opType;
            // read operation type from public data (the first byte per each operation)
            assembly {
                opType := shr(0xf8, mload(pubDataPtr))
            }

            // cheap operations processing
            if (opType == Operations.OpType.Transfer) {
                pubDataPtr += TRANSFER_BYTES;
            } else if (opType == Operations.OpType.Noop) {
                pubDataPtr += NOOP_BYTES;
            } else if (opType == Operations.OpType.TransferToNew) {
                pubDataPtr += TRANSFER_TO_NEW_BYTES;
            } else {
                // other operations processing

                // calculation of public data offset
                uint256 pubdataOffset = pubDataPtr - pubDataStartPtr;

                if (opType == Operations.OpType.Deposit) {
                    bytes memory pubData = Bytes.slice(_publicData, pubdataOffset + 1, DEPOSIT_BYTES - 1);

                    Operations.Deposit memory depositData = Operations.readDepositPubdata(pubData);
                    emitDepositCommitEvent(_blockNumber, depositData);

                    OnchainOperation memory onchainOp = OnchainOperation(
                        Operations.OpType.Deposit,
                        pubData
                    );
                    commitNextPriorityOperation(onchainOp, currentPriorityRequestId);
                    currentPriorityRequestId++;

                    pubDataPtr += DEPOSIT_BYTES;
                } else if (opType == Operations.OpType.PartialExit) {
                    Operations.PartialExit memory data = Operations.readPartialExitPubdata(_publicData, pubdataOffset + 1);

                    bool addToPendingWithdrawalsQueue = true;
                    withdrawalsDataHash = keccak256(abi.encode(withdrawalsDataHash, addToPendingWithdrawalsQueue, data.owner, data.tokenId, data.amount));

                    pubDataPtr += PARTIAL_EXIT_BYTES;
                } else if (opType == Operations.OpType.FullExit) {
                    bytes memory pubData = Bytes.slice(_publicData, pubdataOffset + 1, FULL_EXIT_BYTES - 1);

                    Operations.FullExit memory fullExitData = Operations.readFullExitPubdata(pubData);
                    emitFullExitCommitEvent(_blockNumber, fullExitData);

                    bool addToPendingWithdrawalsQueue = false;
                    withdrawalsDataHash = keccak256(abi.encode(withdrawalsDataHash, addToPendingWithdrawalsQueue, fullExitData.owner, fullExitData.tokenId, fullExitData.amount));

                    OnchainOperation memory onchainOp = OnchainOperation(
                        Operations.OpType.FullExit,
                        pubData
                    );
                    commitNextPriorityOperation(onchainOp, currentPriorityRequestId);
                    currentPriorityRequestId++;

                    pubDataPtr += FULL_EXIT_BYTES;
                } else if (opType == Operations.OpType.ChangePubKey) {
                    require(processedOperationsRequiringEthWitness < _ethWitnessSizes.length, "fcs13"); // eth witness data malformed
                    Operations.ChangePubKey memory op = Operations.readChangePubKeyPubdata(_publicData, pubdataOffset + 1);

                    if (_ethWitnessSizes[processedOperationsRequiringEthWitness] != 0) {
                        bytes memory currentEthWitness = Bytes.slice(_ethWitness, ethWitnessOffset, _ethWitnessSizes[processedOperationsRequiringEthWitness]);

                        bool valid = verifyChangePubkeySignature(currentEthWitness, op.pubKeyHash, op.nonce, op.owner, op.accountId);
                        require(valid, "fpp15"); // failed to verify change pubkey hash signature
                    } else {
                        bool valid = authFacts[op.owner][op.nonce] == keccak256(abi.encodePacked(op.pubKeyHash));
                        require(valid, "fpp16"); // new pub key hash is not authenticated properly
                    }

                    ethWitnessOffset += _ethWitnessSizes[processedOperationsRequiringEthWitness];
                    processedOperationsRequiringEthWitness++;

                    pubDataPtr += CHANGE_PUBKEY_BYTES;
                } else {
                    revert("fpp14"); // unsupported op
                }
            }
        }
        require(pubDataPtr == pubDataEndPtr, "fcs12"); // last chunk exceeds pubdata
        require(ethWitnessOffset == _ethWitness.length, "fcs14"); // _ethWitness was not used completely
        require(processedOperationsRequiringEthWitness == _ethWitnessSizes.length, "fcs15"); // _ethWitnessSizes was not used completely

        require(currentPriorityRequestId <= firstPriorityRequestId + totalOpenPriorityRequests, "fcs16"); // fcs16 - excess priority requests in pubdata
        totalCommittedPriorityRequests = currentPriorityRequestId - firstPriorityRequestId;
    }

    /// @notice Checks that signature is valid for pubkey change message
    /// @param _signature Signature
    /// @param _newPkHash New pubkey hash
    /// @param _nonce Nonce used for message
    /// @param _ethAddress Account's ethereum address
    /// @param _accountId Id of zkSync account
    function verifyChangePubkeySignature(bytes memory _signature, bytes20 _newPkHash, uint32 _nonce, address _ethAddress, uint32 _accountId) internal pure returns (bool) {
        bytes memory signedMessage = abi.encodePacked(
            "\x19Ethereum Signed Message:\n152",
            "Register zkSync pubkey:\n\n",
            Bytes.bytesToHexASCIIBytes(abi.encodePacked(_newPkHash)), "\n",
            "nonce: 0x", Bytes.bytesToHexASCIIBytes(Bytes.toBytesFromUInt32(_nonce)), "\n",
            "account id: 0x", Bytes.bytesToHexASCIIBytes(Bytes.toBytesFromUInt32(_accountId)),
            "\n\n",
            "Only sign this message for a trusted client!"
        );
        address recoveredAddress = Utils.recoverAddressFromEthSignature(_signature, signedMessage);
        return recoveredAddress == _ethAddress;
    }

    /// @notice Creates block commitment from its data
    /// @param _blockNumber Block number
    /// @param _feeAccount Account to collect fees
    /// @param _blockTimestamp Timestamp to be used in the verifier
    /// @param _oldRoot Old tree root
    /// @param _newRoot New tree root
    /// @param _publicData Operations pubdata
    /// @return block commitment
    function createBlockCommitment(
        uint32 _blockNumber,
        uint32 _feeAccount,
        uint64 _blockTimestamp,
        bytes32 _oldRoot,
        bytes32 _newRoot,
        bytes memory _publicData
    ) internal view returns (bytes32 commitment) {
        bytes32 hash = sha256(
            abi.encodePacked(uint256(_blockNumber), uint256(_feeAccount))
        );
        hash = sha256(abi.encodePacked(hash, uint256(_blockTimestamp)));
        hash = sha256(abi.encodePacked(hash, uint256(_oldRoot)));
        hash = sha256(abi.encodePacked(hash, uint256(_newRoot)));

        /// The code below is equivalent to `commitment = sha256(abi.encodePacked(hash, _publicData))`

        /// We use inline assembly instead of this concise and readable code in order to avoid copying of `_publicData` (which saves ~90 gas per transfer operation).

        /// Specifically, we perform the following trick:
        /// First, replace the first 32 bytes of `_publicData` (where normally its length is stored) with the value of `hash`.
        /// Then, we call `sha256` precompile passing the `_publicData` pointer and the length of the concatenated byte buffer.
        /// Finally, we put the `_publicData.length` back to its original location (to the first word of `_publicData`).
        assembly {
            let hashResult := mload(0x40)
            let pubDataLen := mload(_publicData)
            mstore(_publicData, hash)
            // staticcall to the sha256 precompile at address 0x2
            let success := staticcall(
                gas,
                0x2,
                _publicData,
                add(pubDataLen, 0x20),
                hashResult,
                0x20
            )
            mstore(_publicData, pubDataLen)

            // Use "invalid" to make gas estimation work
            switch success case 0 { invalid() }

            commitment := mload(hashResult)
        }
    }

    /// @notice Checks that operation is same as operation in priority queue
    /// @param _onchainOp The operation
    /// @param _priorityRequestId Operation's id in priority queue
    function commitNextPriorityOperation(OnchainOperation memory _onchainOp, uint64 _priorityRequestId) internal view {
        Operations.OpType priorReqType = priorityRequests[_priorityRequestId].opType;
        bytes memory priorReqPubdata = priorityRequests[_priorityRequestId].pubData;

        require(priorReqType == _onchainOp.opType, "nvp12"); // incorrect priority op type

        if (_onchainOp.opType == Operations.OpType.Deposit) {
            require(Operations.depositPubdataMatch(priorReqPubdata, _onchainOp.pubData), "vnp13");
        } else if (_onchainOp.opType == Operations.OpType.FullExit) {
            require(Operations.fullExitPubdataMatch(priorReqPubdata, _onchainOp.pubData), "vnp14");
        } else {
            revert("vnp15"); // invalid or non-priority operation
        }
    }

    /// @notice Processes onchain withdrawals. Full exit withdrawals will not be added to pending withdrawals queue
    /// @dev NOTICE: must process only withdrawals which hash matches with expectedWithdrawalsDataHash.
    /// @param withdrawalsData Withdrawals data
    /// @param expectedWithdrawalsDataHash Expected withdrawals data hash
    function processOnchainWithdrawals(bytes memory withdrawalsData, bytes32 expectedWithdrawalsDataHash) internal {
        require(withdrawalsData.length % ONCHAIN_WITHDRAWAL_BYTES == 0, "pow11"); // pow11 - withdrawalData length is not multiple of ONCHAIN_WITHDRAWAL_BYTES

        bytes32 withdrawalsDataHash = EMPTY_STRING_KECCAK;

        uint offset = 0;
        uint32 localNumberOfPendingWithdrawals = numberOfPendingWithdrawals;
        while (offset < withdrawalsData.length) {
            (bool addToPendingWithdrawalsQueue, address _to, uint16 _tokenId, uint128 _amount) = Operations.readWithdrawalData(withdrawalsData, offset);
            bytes22 packedBalanceKey = packAddressAndTokenId(_to, _tokenId);

            uint128 balance = balancesToWithdraw[packedBalanceKey].balanceToWithdraw;
            // after this all writes to this slot will cost 5k gas
            balancesToWithdraw[packedBalanceKey] = BalanceToWithdraw({
                balanceToWithdraw: balance.add(_amount),
                gasReserveValue: 0xff
            });

            if (addToPendingWithdrawalsQueue) {
                pendingWithdrawals[firstPendingWithdrawalIndex + localNumberOfPendingWithdrawals] = PendingWithdrawal(_to, _tokenId);
                localNumberOfPendingWithdrawals++;
            }

            withdrawalsDataHash = keccak256(abi.encode(withdrawalsDataHash, addToPendingWithdrawalsQueue, _to, _tokenId, _amount));
            offset += ONCHAIN_WITHDRAWAL_BYTES;
        }
        require(withdrawalsDataHash == expectedWithdrawalsDataHash, "pow12"); // pow12 - withdrawals data hash not matches with expected value
        if (numberOfPendingWithdrawals != localNumberOfPendingWithdrawals) {
            emit PendingWithdrawalsAdd(firstPendingWithdrawalIndex + numberOfPendingWithdrawals, firstPendingWithdrawalIndex + localNumberOfPendingWithdrawals);
        }
        numberOfPendingWithdrawals = localNumberOfPendingWithdrawals;
    }

    /// @notice Checks whether oldest unverified block has expired
    /// @return bool flag that indicates whether oldest unverified block has expired
    function isBlockCommitmentExpired() internal view returns (bool) {
        return (
            totalBlocksCommitted > totalBlocksVerified &&
            blocks[totalBlocksVerified + 1].committedAtBlock > 0 &&
            block.number > blocks[totalBlocksVerified + 1].committedAtBlock + EXPECT_VERIFICATION_IN
        );
    }

    /// @notice Deletes processed priority requests
    /// @param _number The number of requests
    function deleteRequests(uint64 _number) internal {
        require(_number <= totalOpenPriorityRequests, "pcs21"); // number is higher than total priority requests number

        uint64 numberOfRequestsToClear = Utils.minU64(_number, MAX_PRIORITY_REQUESTS_TO_DELETE_IN_VERIFY);
        uint64 startIndex = firstPriorityRequestId;
        for (uint64 i = startIndex; i < startIndex + numberOfRequestsToClear; i++) {
            delete priorityRequests[i];
        }

        totalOpenPriorityRequests -= _number;
        firstPriorityRequestId += _number;
        totalCommittedPriorityRequests -= _number;
    }

    ///
    /// External function's to allow testing some of the internal functional
    ///

    function externalTestVerifyChangePubkeySignature(bytes calldata _signature, bytes20 _newPkHash, uint32 _nonce, address _ethAddress, uint32 _accountId) external returns (bool) {
        return verifyChangePubkeySignature(_signature, _newPkHash, _nonce, _ethAddress, _accountId);
    }

    function externalTestCollectOnchainOps(uint32 _blockNumber, bytes calldata _publicData, bytes calldata _ethWitness, uint32[] calldata _ethWitnessSizes)
        external returns (bytes32 withdrawalsDataHash) {
        return collectOnchainOps(_blockNumber, _publicData, _ethWitness, _ethWitnessSizes);
    }
}
