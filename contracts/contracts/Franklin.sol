pragma solidity 0.5.16;

import "../node_modules/openzeppelin-solidity/contracts/token/ERC20/IERC20.sol";

import "./Storage.sol";
import "./Config.sol";
import "./Events.sol";

import "./Bytes.sol";
import "./Operations.sol";


/// @title zkSync main contract
/// @author Matter Labs
contract Franklin is Storage, Config, Events {

    // // Migration

    // // Address of the new version of the contract to migrate accounts to
    // // Can be proposed by network governor
    // address public migrateTo;

    // // Migration deadline: after this ETH block number migration may happen with the contract
    // // entering exodus mode for all users who have not opted in for migration
    // uint32  public migrateByBlock;

    // // Flag for the new contract to indicate that the migration has been sealed
    // bool    public migrationSealed;

    // mapping (uint32 => bool) tokenMigrated;

    /// @notice Constructs Franklin contract
    /// @param _governanceAddress The address of Governance contract
    /// @param _verifierAddress The address of Verifier contract
    /// _genesisAccAddress The address of single account, that exists in genesis block
    /// @param _genesisRoot Genesis blocks (first block) root
    constructor(
        address _governanceAddress,
        address _verifierAddress,
        address, // FIXME: remove _genesisAccAddress
        bytes32 _genesisRoot
    ) public {
        verifier = Verifier(_verifierAddress);
        governance = Governance(_governanceAddress);

        blocks[0].stateRoot = _genesisRoot;
    }

    /// @notice executes pending withdrawals
    /// @param _n The number of withdrawals to complete starting from oldest
    function completeWithdrawals(uint32 _n) external {
        // TODO: when switched to multi validators model we need to add incentive mechanism to call complete.
        uint32 toProcess = _n;
        if (toProcess > numberOfPendingWithdrawals) {
            toProcess = numberOfPendingWithdrawals;
        }
        uint32 startIndex = firstPendingWithdrawalIndex;
        numberOfPendingWithdrawals -= toProcess;
        if (numberOfPendingWithdrawals == 0) {
            firstPendingWithdrawalIndex = 0;
        } else {
            firstPendingWithdrawalIndex += toProcess;
        }

        for (uint32 i = startIndex; i < startIndex + toProcess; ++i) {
            uint16 tokenId = pendingWithdrawals[i].tokenId;
            address to = pendingWithdrawals[i].to;
            // send fails are ignored hence there is always a direct way to withdraw.
            delete pendingWithdrawals[i];

            uint128 amount = balancesToWithdraw[to][tokenId];
            // amount is zero means funds has been withdrawn with withdrawETH or withdrawERC20
            if (amount != 0) {
                // avoid reentrancy attack by using subtract and not "= 0" and changing local state before external call
                balancesToWithdraw[to][tokenId] -= amount;
                bool sent = false;
                if (tokenId == 0) {
                    address payable toPayable = address(uint160(to));
                    sent = toPayable.send(amount);
                } else {
                    address tokenAddr = governance.tokenAddresses(tokenId);
                    require(tokenAddr != address(0), "cwd11"); // unknown tokenId
                    sent = IERC20(tokenAddr).transfer(to, amount);
                }
                if (!sent) {
                    balancesToWithdraw[to][tokenId] += amount;
                }
            }
        }
    }

    function minU32(uint32 a, uint32 b) internal pure returns (uint32) {
        return a < b ? a : b;
    }

    function minU64(uint64 a, uint64 b) internal pure returns (uint64) {
        return a < b ? a : b;
    }

    /// @notice Accrues users balances from deposit priority requests in Exodus mode
    /// @dev WARNING: Only for Exodus mode
    /// @dev Canceling may take several separate transactions to be completed
    /// @param _requests number of requests to process
    function cancelOutstandingDepositsForExodusMode(uint64 _requests) external {
        require(exodusMode, "coe01"); // exodus mode not active
        require(_requests > 0, "coe02"); // provided zero number of requests
        require(totalOpenPriorityRequests > 0, "coe03"); // no priority requests left
        uint64 toProcess = minU64(totalOpenPriorityRequests, _requests);
        for (uint64 i = 0; i < toProcess; i++) {
            uint64 id = firstPriorityRequestId + i;
            if (priorityRequests[id].opType == Operations.OpType.Deposit) {
                ( , Operations.Deposit memory op) = Operations.readDepositPubdata(priorityRequests[id].pubData, 0);
                balancesToWithdraw[op.owner][op.tokenId] += op.amount;
            }
            delete priorityRequests[id];
        }
        firstPriorityRequestId += toProcess;
        totalOpenPriorityRequests -= toProcess;
    }

    // function scheduleMigration(address _migrateTo, uint32 _migrateByBlock) external {
    //     requireGovernor();
    //     require(migrateByBlock == 0, "migration in progress");
    //     migrateTo = _migrateTo;
    //     migrateByBlock = _migrateByBlock;
    // }

    // // Anybody MUST be able to call this function
    // function sealMigration() external {
    //     require(migrateByBlock > 0, "no migration scheduled");
    //     migrationSealed = true;
    //     exodusMode = true;
    // }

    // // Anybody MUST be able to call this function
    // function migrateToken(uint32 _tokenId, uint128 /*_amount*/, bytes calldata /*_proof*/) external {
    //     require(migrationSealed, "migration not sealed");
    //     requireValidToken(_tokenId);
    //     require(tokenMigrated[_tokenId]==false, "token already migrated");
    //     // TODO: check the proof for the amount
    //     // TODO: transfer ERC20 or ETH to the `migrateTo` address
    //     tokenMigrated[_tokenId] = true;

    //     require(false, "unimplemented");
    // }

    /// @notice Deposit ETH to Layer 2 - transfer ether from user into contract, validate it, register deposit
    /// @param _amount Amount to deposit (if user specified msg.value more than this amount + fee - she will receive difference)
    /// @param _franklinAddr The receiver Layer 2 address
    function depositETH(uint128 _amount, address _franklinAddr) external payable {
        requireActive();

        // Fee is:
        //   fee coeff * base tx gas cost * gas price
        uint fee = FEE_GAS_PRICE_MULTIPLIER * BASE_DEPOSIT_ETH_GAS * tx.gasprice;

        uint totalValue = fee + _amount;

        require(msg.value >= totalValue, "fdh11"); // Not enough ETH provided

        if (msg.value != totalValue) {
            uint refund = msg.value - totalValue;

            // Doublecheck to never refund more than received!
            require(refund < msg.value, "fdh12");
            msg.sender.transfer(refund);
        }

        registerDeposit(0, _amount, fee, _franklinAddr);
    }

    /// @notice Withdraw ETH to Layer 1 - register withdrawal and transfer ether to sender
    /// @param _amount Ether amount to withdraw
    function withdrawETH(uint128 _amount) external {
        registerSingleWithdrawal(0, _amount);
        msg.sender.transfer(_amount);
    }

    /// @notice Deposit ERC20 token to Layer 2 - transfer ERC20 tokens from user into contract, validate it, register deposit
    /// @param _token Token address
    /// @param _amount Token amount
    /// @param _franklinAddr Receiver Layer 2 address
    function depositERC20(address _token, uint128 _amount, address _franklinAddr) external payable {
        requireActive();

        // Fee is:
        //   fee coeff * base tx gas cost * gas price
        uint256 fee = FEE_GAS_PRICE_MULTIPLIER * BASE_DEPOSIT_ERC_GAS * tx.gasprice;

        // Get token id by its address
        uint16 tokenId = governance.validateTokenAddress(_token);

        require(IERC20(_token).transferFrom(msg.sender, address(this), _amount), "fd012"); // token transfer failed deposit

        registerDeposit(tokenId, _amount, fee, _franklinAddr);

        require(msg.value >= fee, "fd011"); // Not enough ETH provided to pay the fee
        if (msg.value != fee) {
            msg.sender.transfer(msg.value - fee);
        }
    }

    /// @notice Withdraw ERC20 token to Layer 1 - register withdrawal and transfer ERC20 to sender
    /// @param _token Token address
    /// @param _amount amount to withdraw
    function withdrawERC20(address _token, uint128 _amount) external {
        uint16 tokenId = governance.validateTokenAddress(_token);
        registerSingleWithdrawal(tokenId, _amount);
        require(IERC20(_token).transfer(msg.sender, _amount), "fw011"); // token transfer failed withdraw
    }

    /// @notice Register full exit request - pack pubdata, add priority request
    /// @param _accountId Numerical id of the account
    /// @param _token Token address, 0 address for ether
    function fullExit (uint24 _accountId, address _token) external payable {
        requireActive();

        // Fee is:
        //   fee coeff * base tx gas cost * gas price
        uint256 fee = FEE_GAS_PRICE_MULTIPLIER * BASE_FULL_EXIT_GAS * tx.gasprice;

        uint16 tokenId;
        if (_token == address(0)) {
            tokenId = 0;
        } else {
            tokenId = governance.validateTokenAddress(_token);
        }

        // Priority Queue request
        Operations.FullExit memory op = Operations.FullExit({
            accountId:  _accountId,
            owner:      msg.sender,
            tokenId:    tokenId,
            amount:     0 // unknown at this point
        });
        bytes memory pubData = Operations.writeFullExitPubdata(op);
        addPriorityRequest(Operations.OpType.FullExit, fee, pubData);

        require(msg.value >= fee, "fft11"); // Not enough ETH provided to pay the fee
        if (msg.value != fee) {
            msg.sender.transfer(msg.value-fee);
        }
    }

    /// @notice Register deposit request - pack pubdata, add priority request and emit OnchainDeposit event
    /// @param _token Token by id
    /// @param _amount Token amount
    /// @param _fee Validator fee
    /// @param _owner Receiver
    function registerDeposit(
        uint16 _token,
        uint128 _amount,
        uint256 _fee,
        address _owner
    ) internal {
        require(governance.isValidTokenId(_token), "rgd11"); // invalid token id

        // Priority Queue request
        Operations.Deposit memory op = Operations.Deposit({
            owner:      _owner,
            tokenId:    _token,
            amount:     _amount
        });
        bytes memory pubData = Operations.writeDepositPubdata(op);
        addPriorityRequest(Operations.OpType.Deposit, _fee, pubData);

        emit OnchainDeposit(
            msg.sender,
            _token,
            _amount,
            _fee,
            _owner
        );
    }

    /// @notice Register withdrawal - update user balances and emit OnchainWithdrawal event
    /// @param _token - token by id
    /// @param _amount - token amount
    function registerSingleWithdrawal(uint16 _token, uint128 _amount) internal {
        require(balancesToWithdraw[msg.sender][_token] >= _amount, "frw11"); // insufficient balance withdraw
        balancesToWithdraw[msg.sender][_token] -= _amount;
        emit OnchainWithdrawal(
            msg.sender,
            _token,
            _amount
        );
    }

    /// @notice Commit block - collect onchain operations, create its commitment, emit BlockCommitted event
    /// @param _blockNumber Block number
    /// @param _feeAccount Account to collect fees
    /// @param _newRoot New tree root
    /// @param _publicData Operations pubdata
    /// @param _ethWitness Data passed to ethereum outside pubdata of the circuit.
    /// @param _ethWitnessSizes Amount of eth witness bytes for the corresponding operation.
    ///
    /// _blockNumber is not necessary but it may help to catch server-side errors.
    function commitBlock(
        uint32 _blockNumber,
        uint24 _feeAccount,
        bytes32 _newRoot,
        bytes calldata _publicData,
        bytes calldata _ethWitness,
        uint32[] calldata _ethWitnessSizes
    ) external {
        requireActive();
        require(_blockNumber == totalBlocksCommitted + 1, "fck11"); // only commit next block
        governance.requireActiveValidator(msg.sender);
        require(!isBlockCommitmentExpired(), "fck12"); // committed blocks had expired
        if(!triggerExodusIfNeeded()) {
            require(totalBlocksCommitted - totalBlocksVerified < MAX_UNVERIFIED_BLOCKS, "fck13"); // too many blocks committed

            // Unpack onchain operations and store them.
            // Get onchain operations start id for global onchain operations counter,
            // onchain operations number for this block, priority operations number for this block.
            uint64 firstOnchainOpId = totalOnchainOps;
            uint64 prevTotalCommittedPriorityRequests = totalCommittedPriorityRequests;

            uint64 nOnchainOpsProcessed = collectOnchainOps(_publicData, _ethWitness, _ethWitnessSizes);

            uint64 nPriorityRequestProcessed = totalCommittedPriorityRequests - prevTotalCommittedPriorityRequests;

            createCommittedBlock(_blockNumber, _feeAccount, _newRoot, _publicData, firstOnchainOpId, nOnchainOpsProcessed, nPriorityRequestProcessed);
            totalBlocksCommitted++;

            emit BlockCommitted(_blockNumber);
        }
    }

    /// @notice Store committed block structure to the storage.
    /// @param _firstOnchainOpId - blocks' onchain ops start id in global operations
    /// @param _nOnchainOpsProcessed - total number of onchain ops in block
    /// @param _nCommittedPriorityRequests - total number of priority requests in block
    function createCommittedBlock(
        uint32 _blockNumber,
        uint24 _feeAccount,
        bytes32 _newRoot,
        bytes memory _publicData,
        uint64 _firstOnchainOpId, uint64 _nOnchainOpsProcessed, uint64 _nCommittedPriorityRequests
    ) internal {
        require(_publicData.length % 8 == 0, "cbb10"); // Public data size is not multiple of 8

        uint32 blockChunks = uint32(_publicData.length / 8);
        require(verifier.isBlockSizeSupported(blockChunks), "ccb11");

        // Create block commitment for verification proof
        bytes32 commitment = createBlockCommitment(
            _blockNumber,
            _feeAccount,
            blocks[_blockNumber - 1].stateRoot,
            _newRoot,
            _publicData
        );

        blocks[_blockNumber] = Block(
            msg.sender, // validator
            uint32(block.number), // committed at
            _firstOnchainOpId, // blocks' onchain ops start id in global operations
            _nOnchainOpsProcessed, // total number of onchain ops in block
            _nCommittedPriorityRequests, // total number of priority onchain ops in block
            commitment, // blocks' commitment
            _newRoot, // new root
            blockChunks
        );
    }

    /// @notice Gets operations packed in bytes array. Unpacks it and stores onchain operations.
    /// @param _publicData Operations packed in bytes array
    /// @param _ethWitness Eth witness that was posted with commit
    /// @param _ethWitnessSizes Amount of eth witness bytes for the corresponding operation.
    function collectOnchainOps(bytes memory _publicData, bytes memory _ethWitness, uint32[] memory _ethWitnessSizes)
        internal returns (uint32 _processedZKSyncOperation){
        require(_publicData.length % 8 == 0, "fcs11"); // pubdata length must be a multiple of 8 because each chunk is 8 bytes

        uint256 pubdataOffset = 0;

        uint64 ethWitnessOffset = 0;
        uint32 processedZKSyncOperation = 0;

        while (pubdataOffset < _publicData.length) {
            require(processedZKSyncOperation < _ethWitnessSizes.length, "fcs13"); // eth witness data malformed
            bytes memory zksyncOperationETHWitness = Bytes.slice(_ethWitness, ethWitnessOffset, _ethWitnessSizes[processedZKSyncOperation]);

            pubdataOffset += processNextOperation(
                pubdataOffset,
                _publicData,
                zksyncOperationETHWitness
            );

            ethWitnessOffset += _ethWitnessSizes[processedZKSyncOperation];
            processedZKSyncOperation++;
        }
        require(pubdataOffset == _publicData.length, "fcs12"); // last chunk exceeds pubdata
        require(ethWitnessOffset == _ethWitness.length, "fcs14"); // _ethWitness was not used completely

        return processedZKSyncOperation;
    }

    /// @notice Verifies ethereum signature for given message and recovers address of the signer
    /// @param _signature 65 bytes concatenated. R (32) + S (32) + V (1)
    /// @param _message signed message.
    /// @return address of the signer
    function verifyEthereumSignature(bytes memory _signature, bytes memory _message) internal pure returns (address) {
        require(_signature.length == 2*ETH_SIGN_RS_BYTES + 1, "ves10"); // incorrect signature length

        uint offset = 0;
        bytes32 signR = Bytes.bytesToBytes32(Bytes.slice(_signature, offset, ETH_SIGN_RS_BYTES));
        offset += ETH_SIGN_RS_BYTES;
        bytes32 signS = Bytes.bytesToBytes32(Bytes.slice(_signature, offset, ETH_SIGN_RS_BYTES));
        offset += ETH_SIGN_RS_BYTES;
        uint8 signV = uint8(_signature[offset]);

        return ecrecover(keccak256(_message), signV, signR, signS);
    }

    function verifyChangePubkeySignature(bytes memory _signature, bytes memory _newPkHash, uint32 _nonce, address _ethAddress) internal pure returns (bool) {
        require(_newPkHash.length == 20, "vpk11"); // unexpected hash length

        bytes memory signedMessage = abi.encodePacked(
            "\x19Ethereum Signed Message:\n135",
            "Register ZK Sync pubkey:\n\n",
            "sync:", Bytes.bytesToHexASCIIBytes(_newPkHash),
            " nonce: 0x", Bytes.bytesToHexASCIIBytes(Bytes.toBytesFromUInt32(_nonce)),
            "\n\n",
            "Only sign this message for a trusted client!"
        );
        address recoveredAddress = verifyEthereumSignature(_signature, signedMessage);
        return recoveredAddress == _ethAddress;
    }

    /// @notice On the first byte determines the type of operation, if it is an onchain operation - saves it in storage
    /// @param _pubdataOffset Current offset in pubdata
    /// @param _publicData Operation pubdata
    /// @param _currentEthWitness current eth witness for operation
    /// @return pubdata bytes processed
    function processNextOperation(
        uint256 _pubdataOffset,
        bytes memory _publicData,
        bytes memory _currentEthWitness
    ) internal returns (uint256 _bytesProcessed) {
        Operations.OpType opType = Operations.OpType(uint8(_publicData[_pubdataOffset]));

        if (opType == Operations.OpType.Noop) return NOOP_BYTES;
        if (opType == Operations.OpType.TransferToNew) return TRANSFER_TO_NEW_BYTES;
        if (opType == Operations.OpType.Transfer) return TRANSFER_BYTES;
        if (opType == Operations.OpType.CloseAccount) return CLOSE_ACCOUNT_BYTES;

        if (opType == Operations.OpType.Deposit) {
            bytes memory pubData = Bytes.slice(_publicData, _pubdataOffset + 1, DEPOSIT_BYTES - 1);
            onchainOps[totalOnchainOps] = OnchainOperation(
                Operations.OpType.Deposit,
                pubData
            );
            verifyNextPriorityOperation(onchainOps[totalOnchainOps]);

            totalOnchainOps++;

            return DEPOSIT_BYTES;
        }

        if (opType == Operations.OpType.PartialExit) {
            bytes memory pubData = Bytes.slice(_publicData, _pubdataOffset + 1, PARTIAL_EXIT_BYTES - 1);
            onchainOps[totalOnchainOps] = OnchainOperation(
                Operations.OpType.PartialExit,
                pubData
            );
            totalOnchainOps++;

            return PARTIAL_EXIT_BYTES;
        }

        if (opType == Operations.OpType.FullExit) {
            bytes memory pubData = Bytes.slice(_publicData, _pubdataOffset + 1, FULL_EXIT_BYTES - 1);
            onchainOps[totalOnchainOps] = OnchainOperation(
                Operations.OpType.FullExit,
                pubData
            );

            verifyNextPriorityOperation(onchainOps[totalOnchainOps]);

            totalOnchainOps++;
            return FULL_EXIT_BYTES;
        }

        if (opType == Operations.OpType.ChangePubKey) {
            Operations.ChangePubKey memory op = Operations.readChangePubKeyPubdata(_publicData, _pubdataOffset + 1);
            if (_currentEthWitness.length > 0) {
                bool valid = verifyChangePubkeySignature(_currentEthWitness, op.pubKeyHash, op.nonce, op.owner);
                require(valid, "fpp15"); // failed to verify change pubkey hash signature
            } else {
                bytes memory authFact = authFacts[op.owner][op.nonce];
                require(authFact.length == op.pubKeyHash.length, "fpp17");  // invalid authFact length
                bool valid = Bytes.isEqualSlices(authFact, 0, op.pubKeyHash, 0, op.pubKeyHash.length);
                require(valid, "fpp16"); // new pub key hash is not authenticated properly
            }
            return CHANGE_PUBKEY_BYTES;
        }

        revert("fpp14"); // unsupported op
    }

    /// @notice Creates block commitment from its data
    /// @param _blockNumber Block number
    /// @param _feeAccount Account to collect fees
    /// @param _oldRoot Old tree root
    /// @param _newRoot New tree root
    /// @param _publicData Operations pubdata
    /// @return block commitment
    function createBlockCommitment(
        uint32 _blockNumber,
        uint24 _feeAccount,
        bytes32 _oldRoot,
        bytes32 _newRoot,
        bytes memory _publicData
    ) internal pure returns (bytes32) {
        bytes32 hash = sha256(
            abi.encodePacked(uint256(_blockNumber), uint256(_feeAccount))
        );
        hash = sha256(abi.encodePacked(hash, uint256(_oldRoot)));
        hash = sha256(abi.encodePacked(hash, uint256(_newRoot)));
        // public data is committed with padding (TODO: check assembly and optimize to avoid copying data)
        hash = sha256(
            abi.encodePacked(
                hash,
                _publicData
            )
        );
        return hash;
    }

    function verifyNextPriorityOperation(OnchainOperation memory _onchainOp) internal {
        require(totalOpenPriorityRequests > totalCommittedPriorityRequests, "vnp11"); // no more priority requests in queue

        uint64 _priorityRequestId = firstPriorityRequestId + totalCommittedPriorityRequests;
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

        totalCommittedPriorityRequests++;
    }

    /// @notice Removes some onchain ops (for example in case of wrong priority comparison)
    /// @param _startId Onchain op start id
    /// @param _totalProcessed How many ops are procceeded
    function revertOnchainOps(uint64 _startId, uint64 _totalProcessed) internal {
        uint64 start = _startId;
        uint64 end = start + _totalProcessed;

        for (uint64 current = start; current < end; ++current) {
            delete onchainOps[current];
        }
    }

    /// @notice Block verification.
    /// @notice Verify proof -> consummate onchain ops (accrue balances from withdrawals) -> remove priority requests
    /// @param _blockNumber Block number
    /// @param _proof Block proof
    function verifyBlock(uint32 _blockNumber, uint256[8] calldata _proof)
        external
    {
        requireActive();
        require(_blockNumber == totalBlocksVerified + 1, "fvk11"); // only verify next block
        governance.requireActiveValidator(msg.sender);

        require(verifier.verifyBlockProof(_proof, blocks[_blockNumber].commitment, blocks[_blockNumber].chunks), "fvk13"); // proof verification failed

        consummateOnchainOps(_blockNumber);

        collectValidatorsFeeAndDeleteRequests(
            blocks[_blockNumber].priorityOperations,
            blocks[_blockNumber].validator
        );

        totalBlocksVerified += 1;

        emit BlockVerified(_blockNumber);
    }

    /// @notice When block with withdrawals is verified we store them and complete in separate tx. Withdrawals can be complete by calling withdrawEth, withdrawERC20 or completeWithdrawals.
    /// @param _to Receiver
    /// @param _tokenId Token id
    /// @param _amount Token amount
    function storeWithdrawalAsPending(address _to, uint16 _tokenId, uint128 _amount) internal {
        pendingWithdrawals[firstPendingWithdrawalIndex + numberOfPendingWithdrawals] = PendingWithdrawal(_to, _tokenId);
        numberOfPendingWithdrawals++;

        balancesToWithdraw[_to][_tokenId] += _amount;
    }

    /// @notice If block is verified the onchain operations from it must be completed
    /// @notice (user must have possibility to withdraw funds if withdrawed)
    /// @param _blockNumber Number of block
    function consummateOnchainOps(uint32 _blockNumber) internal {
        uint64 start = blocks[_blockNumber].operationStartId;
        uint64 end = start + blocks[_blockNumber].onchainOperations;
        for (uint64 current = start; current < end; ++current) {
            OnchainOperation memory op = onchainOps[current];
            if (op.opType == Operations.OpType.PartialExit) {
                // partial exit was successful, accrue balance
                Operations.PartialExit memory data = Operations.readPartialExitPubdata(op.pubData, 0);
                storeWithdrawalAsPending(data.owner, data.tokenId, data.amount);
            }
            if (op.opType == Operations.OpType.FullExit) {
                // full exit was successful, accrue balance
                Operations.FullExit memory data = Operations.readFullExitPubdata(op.pubData, 0);
                storeWithdrawalAsPending(data.owner, data.tokenId, data.amount);
            }
            delete onchainOps[current];
        }
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

    /// @notice Reverts unverified blocks
    /// @param _maxBlocksToRevert the maximum number blocks that will be reverted (use if can't revert all blocks because of gas limit).
    function revertBlocks(uint32 _maxBlocksToRevert) external {
        // TODO: limit who can call this method

        require(isBlockCommitmentExpired(), "rbs11"); // trying to revert non-expired blocks.

        uint32 blocksToRevert = minU32(_maxBlocksToRevert, totalBlocksCommitted - totalBlocksVerified);
        uint64 revertedPriorityRequests = 0;
        uint64 revertedOnchainOps = 0;

        for (uint32 i = totalBlocksCommitted - blocksToRevert + 1; i <= totalBlocksCommitted; i++) {
            Block memory revertedBlock = blocks[i];
            require(revertedBlock.committedAtBlock > 0, "frk11"); // block not found
            revertOnchainOps(revertedBlock.operationStartId, revertedBlock.onchainOperations);

            revertedOnchainOps += revertedBlock.onchainOperations;
            revertedPriorityRequests += revertedBlock.priorityOperations;

            delete blocks[i];
        }

        totalBlocksCommitted -= blocksToRevert;
        totalOnchainOps -= revertedOnchainOps;
        totalCommittedPriorityRequests -= revertedPriorityRequests;

        emit BlocksReverted(totalBlocksVerified, totalBlocksCommitted);
    }

    /// @notice Checks that current state not is exodus mode
    function requireActive() internal view {
        require(!exodusMode, "fre11"); // exodus mode activated
    }

    /// @notice Checks if Exodus mode must be entered. If true - cancels outstanding deposits and emits ExodusMode event.
    /// @dev Exodus mode must be entered in case of current ethereum block number is higher than the oldest
    /// @dev of existed priority requests expiration block number.
    /// @return bool flag that is true if the Exodus mode must be entered.
    function triggerExodusIfNeeded() public returns (bool) {
        bool trigger = block.number >= priorityRequests[firstPriorityRequestId].expirationBlock &&
            priorityRequests[firstPriorityRequestId].expirationBlock != 0;
        if (trigger) {
            exodusMode = true;
            emit ExodusMode();
            return true;
        } else {
            return false;
        }
    }

    /// @notice Withdraws token from Franklin to root chain in case of exodus mode. User must provide proof that he owns funds
    /// @param _proof Proof
    /// @param _tokenId Verified token id
    /// @param _amount Amount for owner
    function exit(uint16 _tokenId, uint128 _amount, uint256[8] calldata _proof) external {
        require(exodusMode, "fet11"); // must be in exodus mode
        require(exited[msg.sender][_tokenId] == false, "fet12"); // already exited
        require(verifier.verifyExitProof(blocks[totalBlocksVerified].stateRoot, msg.sender, _tokenId, _amount, _proof), "fet13"); // verification failed

        balancesToWithdraw[msg.sender][_tokenId] += _amount;
        exited[msg.sender][_tokenId] = true;
    }

    function authPubkeyHash(bytes calldata _fact, uint32 _nonce) external {
        require(_fact.length == PUBKEY_HASH_BYTES, "ahf10"); // PubKeyHash should be 20 bytes.
        require(authFacts[msg.sender][_nonce].length == 0, "ahf11"); // auth fact for nonce should be empty

        authFacts[msg.sender][_nonce] = _fact;

        emit FactAuth(msg.sender, _nonce, _fact);
    }

    // Priority queue

        /// @notice Saves priority request in storage
    /// @dev Calculates expiration block for request, store this request and emit NewPriorityRequest event
    /// @param _opType Rollup operation type
    /// @param _fee Validators' fee
    /// @param _pubData Operation pubdata
    function addPriorityRequest(
        Operations.OpType _opType,
        uint256 _fee,
        bytes memory _pubData
    ) internal {
        // Expiration block is: current block number + priority expiration delta
        uint256 expirationBlock = block.number + PRIORITY_EXPIRATION;

        priorityRequests[firstPriorityRequestId+totalOpenPriorityRequests] = PriorityOperation({
            opType: _opType,
            pubData: _pubData,
            expirationBlock: expirationBlock,
            fee: _fee
        });

        emit NewPriorityRequest(
            msg.sender,
            firstPriorityRequestId+totalOpenPriorityRequests,
            uint8(_opType),
            _pubData,
            expirationBlock,
            _fee
        );

        totalOpenPriorityRequests++;
    }

    /// @notice Collects fees from provided requests number for the block validator, store it on her
    /// @notice balance to withdraw in Ether and delete this requests
    /// @param _number The number of requests
    /// @param _validator The address to pay fees
    /// @return validators fee
    function collectValidatorsFeeAndDeleteRequests(uint64 _number, address _validator) internal {
        require(_number <= totalOpenPriorityRequests, "pcs21"); // number is higher than total priority requests number

        uint256 totalFee = 0;
        for (uint64 i = firstPriorityRequestId; i < firstPriorityRequestId + _number; i++) {
            totalFee += priorityRequests[i].fee;
            delete priorityRequests[i];
        }
        totalOpenPriorityRequests -= _number;
        firstPriorityRequestId += _number;
        totalCommittedPriorityRequests -= _number;

        balancesToWithdraw[_validator][0] += uint128(totalFee);
    }

}
