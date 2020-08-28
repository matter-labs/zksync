pragma solidity ^0.5.0;

import "./ReentrancyGuard.sol";
import "./SafeMath.sol";
import "./SafeMathUInt128.sol";
import "./SafeCast.sol";
import "./Utils.sol";

import "./Storage.sol";
import "./Config.sol";
import "./Events.sol";

import "./Bytes.sol";
import "./Operations.sol";

import "./UpgradeableMaster.sol";

/// @title zkSync main contract
/// @author Matter Labs
contract ZkSync is UpgradeableMaster, Storage, Config, Events, ReentrancyGuard {
    using SafeMath for uint256;
    using SafeMathUInt128 for uint128;

    bytes32 public constant EMPTY_STRING_KECCAK = 0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470;

    // Upgrade functional

    /// @notice Notice period before activation preparation status of upgrade mode
    function getNoticePeriod() external returns (uint) {
        return UPGRADE_NOTICE_PERIOD;
    }

    /// @notice Notification that upgrade notice period started
    function upgradeNoticePeriodStarted() external {

    }

    /// @notice Notification that upgrade preparation status is activated
    function upgradePreparationStarted() external {
        upgradePreparationActive = true;
        upgradePreparationActivationTime = now;
    }

    /// @notice Notification that upgrade canceled
    function upgradeCanceled() external {
        upgradePreparationActive = false;
        upgradePreparationActivationTime = 0;
    }

    /// @notice Notification that upgrade finishes
    function upgradeFinishes() external {
        upgradePreparationActive = false;
        upgradePreparationActivationTime = 0;
    }

    /// @notice Checks that contract is ready for upgrade
    /// @return bool flag indicating that contract is ready for upgrade
    function isReadyForUpgrade() external returns (bool) {
        return !exodusMode;
    }

    /// @notice Franklin contract initialization. Can be external because Proxy contract intercepts illegal calls of this function.
    /// @param initializationParameters Encoded representation of initialization parameters:
    /// _governanceAddress The address of Governance contract
    /// _verifierAddress The address of Verifier contract
    /// _ // FIXME: remove _genesisAccAddress
    /// _genesisRoot Genesis blocks (first block) root
    function initialize(bytes calldata initializationParameters) external {
        initializeReentrancyGuard();

        (
        address _governanceAddress,
        address _verifierAddress,
        bytes32 _genesisRoot
        ) = abi.decode(initializationParameters, (address, address, bytes32));

        verifier = Verifier(_verifierAddress);
        governance = Governance(_governanceAddress);

        blocks[0].stateRoot = _genesisRoot;
    }

    /// @notice zkSync contract upgrade. Can be external because Proxy contract intercepts illegal calls of this function.
    /// @param upgradeParameters Encoded representation of upgrade parameters
    function upgrade(bytes calldata upgradeParameters) external {}

    /// @notice Sends tokens
    /// @dev NOTE: will revert if transfer call fails or rollup balance difference (before and after transfer) is bigger than _maxAmount
    /// @param _token Token address
    /// @param _to Address of recipient
    /// @param _amount Amount of tokens to transfer
    /// @param _maxAmount Maximum possible amount of tokens to transfer to this account
    function withdrawERC20Guarded(IERC20 _token, address _to, uint128 _amount, uint128 _maxAmount) external returns (uint128 withdrawnAmount) {
        require(msg.sender == address(this), "wtg10"); // wtg10 - can be called only from this contract as one "external" call (to revert all this function state changes if it is needed)

        uint256 balance_before = _token.balanceOf(address(this));
        require(Utils.sendERC20(_token, _to, _amount), "wtg11"); // wtg11 - ERC20 transfer fails
        uint256 balance_after = _token.balanceOf(address(this));
        uint256 balance_diff = balance_before.sub(balance_after);
        require(balance_diff <= _maxAmount, "wtg12"); // wtg12 - rollup balance difference (before and after transfer) is bigger than _maxAmount

        return SafeCast.toUint128(balance_diff);
    }

    /// @notice executes pending withdrawals
    /// @param _n The number of withdrawals to complete starting from oldest
    function completeWithdrawals(uint32 _n) external nonReentrant {
        // TODO: when switched to multi validators model we need to add incentive mechanism to call complete.
        uint32 toProcess = Utils.minU32(_n, numberOfPendingWithdrawals);
        uint32 startIndex = firstPendingWithdrawalIndex;
        numberOfPendingWithdrawals -= toProcess;
        firstPendingWithdrawalIndex += toProcess;

        for (uint32 i = startIndex; i < startIndex + toProcess; ++i) {
            uint16 tokenId = pendingWithdrawals[i].tokenId;
            address to = pendingWithdrawals[i].to;
            // send fails are ignored hence there is always a direct way to withdraw.
            delete pendingWithdrawals[i];

            bytes22 packedBalanceKey = packAddressAndTokenId(to, tokenId);
            uint128 amount = balancesToWithdraw[packedBalanceKey].balanceToWithdraw;
            // amount is zero means funds has been withdrawn with withdrawETH or withdrawERC20
            if (amount != 0) {
                balancesToWithdraw[packedBalanceKey].balanceToWithdraw -= amount;
                bool sent = false;
                if (tokenId == 0) {
                    address payable toPayable = address(uint160(to));
                    sent = Utils.sendETHNoRevert(toPayable, amount);
                } else {
                    address tokenAddr = governance.tokenAddresses(tokenId);
                    // we can just check that call not reverts because it wants to withdraw all amount
                    (sent, ) = address(this).call.gas(ERC20_WITHDRAWAL_GAS_LIMIT)(
                        abi.encodeWithSignature("withdrawERC20Guarded(address,address,uint128,uint128)", tokenAddr, to, amount, amount)
                    );
                }
                if (!sent) {
                    balancesToWithdraw[packedBalanceKey].balanceToWithdraw += amount;
                }
            }
        }
        if (toProcess > 0) {
            emit PendingWithdrawalsComplete(startIndex, startIndex + toProcess);
        }
    }

    /// @notice Accrues users balances from deposit priority requests in Exodus mode
    /// @dev WARNING: Only for Exodus mode
    /// @dev Canceling may take several separate transactions to be completed
    /// @param _n number of requests to process
    function cancelOutstandingDepositsForExodusMode(uint64 _n) external nonReentrant {
        require(exodusMode, "coe01"); // exodus mode not active
        uint64 toProcess = Utils.minU64(totalOpenPriorityRequests, _n);
        require(toProcess > 0, "coe02"); // no deposits to process
        for (uint64 id = firstPriorityRequestId; id < firstPriorityRequestId + toProcess; id++) {
            if (priorityRequests[id].opType == Operations.OpType.Deposit) {
                Operations.Deposit memory op = Operations.readDepositPubdata(priorityRequests[id].pubData);
                bytes22 packedBalanceKey = packAddressAndTokenId(op.owner, op.tokenId);
                balancesToWithdraw[packedBalanceKey].balanceToWithdraw += op.amount;
            }
            delete priorityRequests[id];
        }
        firstPriorityRequestId += toProcess;
        totalOpenPriorityRequests -= toProcess;
    }

    /// @notice Deposit ETH to Layer 2 - transfer ether from user into contract, validate it, register deposit
    /// @param _franklinAddr The receiver Layer 2 address
    function depositETH(address _franklinAddr) external payable nonReentrant {
        requireActive();
        registerDeposit(0, SafeCast.toUint128(msg.value), _franklinAddr);
    }

    /// @notice Withdraw ETH to Layer 1 - register withdrawal and transfer ether to sender
    /// @param _amount Ether amount to withdraw
    function withdrawETH(uint128 _amount) external nonReentrant {
        registerWithdrawal(0, _amount, msg.sender);
        (bool success, ) = msg.sender.call.value(_amount)("");
        require(success, "fwe11"); // ETH withdraw failed
    }

    /// @notice Deposit ERC20 token to Layer 2 - transfer ERC20 tokens from user into contract, validate it, register deposit
    /// @param _token Token address
    /// @param _amount Token amount
    /// @param _franklinAddr Receiver Layer 2 address
    function depositERC20(IERC20 _token, uint104 _amount, address _franklinAddr) external nonReentrant {
        requireActive();

        // Get token id by its address
        uint16 tokenId = governance.validateTokenAddress(address(_token));

        uint256 balance_before = _token.balanceOf(address(this));
        require(Utils.transferFromERC20(_token, msg.sender, address(this), SafeCast.toUint128(_amount)), "fd012"); // token transfer failed deposit
        uint256 balance_after = _token.balanceOf(address(this));
        uint128 deposit_amount = SafeCast.toUint128(balance_after.sub(balance_before));

        registerDeposit(tokenId, deposit_amount, _franklinAddr);
    }

    /// @notice Withdraw ERC20 token to Layer 1 - register withdrawal and transfer ERC20 to sender
    /// @param _token Token address
    /// @param _amount amount to withdraw
    function withdrawERC20(IERC20 _token, uint128 _amount) external nonReentrant {
        uint16 tokenId = governance.validateTokenAddress(address(_token));
        bytes22 packedBalanceKey = packAddressAndTokenId(msg.sender, tokenId);
        uint128 balance = balancesToWithdraw[packedBalanceKey].balanceToWithdraw;
        uint128 withdrawnAmount = this.withdrawERC20Guarded(_token, msg.sender, _amount, balance);
        registerWithdrawal(tokenId, withdrawnAmount, msg.sender);
    }

    /// @notice Register full exit request - pack pubdata, add priority request
    /// @param _accountId Numerical id of the account
    /// @param _token Token address, 0 address for ether
    function fullExit (uint32 _accountId, address _token) external nonReentrant {
        requireActive();
        require(_accountId <= MAX_ACCOUNT_ID, "fee11");

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
        addPriorityRequest(Operations.OpType.FullExit, pubData);

        // User must fill storage slot of balancesToWithdraw(msg.sender, tokenId) with nonzero value
        // In this case operator should just overwrite this slot during confirming withdrawal
        bytes22 packedBalanceKey = packAddressAndTokenId(msg.sender, tokenId);
        balancesToWithdraw[packedBalanceKey].gasReserveValue = 0xff;
    }

    /// @notice Commit block - collect onchain operations, create its commitment, emit BlockCommit event
    /// @param _blockNumber Block number
    /// @param _feeAccount Account to collect fees
    /// @param _newBlockInfo New state of the block. (first element is the account tree root hash, rest of the array is reserved for the future)
    /// @param _publicData Operations pubdata
    /// @param _ethWitness Data passed to ethereum outside pubdata of the circuit.
    /// @param _ethWitnessSizes Amount of eth witness bytes for the corresponding operation.
    function commitBlock(
        uint32 _blockNumber,
        uint32 _feeAccount,
        bytes32[] calldata _newBlockInfo,
        bytes calldata _publicData,
        bytes calldata _ethWitness,
        uint32[] calldata _ethWitnessSizes
    ) external nonReentrant {
        requireActive();
        require(_blockNumber == totalBlocksCommitted + 1, "fck11"); // only commit next block
        governance.requireActiveValidator(msg.sender);
        require(_newBlockInfo.length == 1, "fck13"); // This version of the contract expects only account tree root hash

        bytes memory publicData = _publicData;

        // Unpack onchain operations and store them.
        // Get priority operations number for this block.
        uint64 prevTotalCommittedPriorityRequests = totalCommittedPriorityRequests;

        bytes32 withdrawalsDataHash = collectOnchainOps(_blockNumber, publicData, _ethWitness, _ethWitnessSizes);

        uint64 nPriorityRequestProcessed = totalCommittedPriorityRequests - prevTotalCommittedPriorityRequests;

        createCommittedBlock(_blockNumber, _feeAccount, _newBlockInfo[0], publicData, withdrawalsDataHash, nPriorityRequestProcessed);
        totalBlocksCommitted++;

        emit BlockCommit(_blockNumber);
    }

    /// @notice Block verification.
    /// @notice Verify proof -> process onchain withdrawals (accrue balances from withdrawals) -> remove priority requests
    /// @param _blockNumber Block number
    /// @param _proof Block proof
    /// @param _withdrawalsData Block withdrawals data
    function verifyBlock(uint32 _blockNumber, uint256[] calldata _proof, bytes calldata _withdrawalsData)
        external nonReentrant
    {
        requireActive();
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
    function revertBlocks(uint32 _maxBlocksToRevert) external nonReentrant {
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

    /// @notice Checks if Exodus mode must be entered. If true - enters exodus mode and emits ExodusMode event.
    /// @dev Exodus mode must be entered in case of current ethereum block number is higher than the oldest
    /// @dev of existed priority requests expiration block number.
    /// @return bool flag that is true if the Exodus mode must be entered.
    function triggerExodusIfNeeded() external returns (bool) {
        bool trigger = block.number >= priorityRequests[firstPriorityRequestId].expirationBlock &&
        priorityRequests[firstPriorityRequestId].expirationBlock != 0;
        if (trigger) {
            if (!exodusMode) {
                exodusMode = true;
                emit ExodusMode();
            }
            return true;
        } else {
            return false;
        }
    }

    /// @notice Withdraws token from Franklin to root chain in case of exodus mode. User must provide proof that he owns funds
    /// @param _accountId Id of the account in the tree
    /// @param _proof Proof
    /// @param _tokenId Verified token id
    /// @param _amount Amount for owner (must be total amount, not part of it)
    function exit(uint32 _accountId, uint16 _tokenId, uint128 _amount, uint256[] calldata _proof) external nonReentrant {
        bytes22 packedBalanceKey = packAddressAndTokenId(msg.sender, _tokenId);
        require(exodusMode, "fet11"); // must be in exodus mode
        require(!exited[_accountId][_tokenId], "fet12"); // already exited
        require(verifier.verifyExitProof(blocks[totalBlocksVerified].stateRoot, _accountId, msg.sender, _tokenId, _amount, _proof), "fet13"); // verification failed

        uint128 balance = balancesToWithdraw[packedBalanceKey].balanceToWithdraw;
        balancesToWithdraw[packedBalanceKey].balanceToWithdraw = balance.add(_amount);
        exited[_accountId][_tokenId] = true;
    }

    function setAuthPubkeyHash(bytes calldata _pubkey_hash, uint32 _nonce) external nonReentrant {
        require(_pubkey_hash.length == PUBKEY_HASH_BYTES, "ahf10"); // PubKeyHash should be 20 bytes.
        require(authFacts[msg.sender][_nonce] == bytes32(0), "ahf11"); // auth fact for nonce should be empty

        authFacts[msg.sender][_nonce] = keccak256(_pubkey_hash);

        emit FactAuth(msg.sender, _nonce, _pubkey_hash);
    }

    /// @notice Register deposit request - pack pubdata, add priority request and emit OnchainDeposit event
    /// @param _tokenId Token by id
    /// @param _amount Token amount
    /// @param _owner Receiver
    function registerDeposit(
        uint16 _tokenId,
        uint128 _amount,
        address _owner
    ) internal {
        // Priority Queue request
        Operations.Deposit memory op = Operations.Deposit({
            accountId:  0, // unknown at this point
            owner:      _owner,
            tokenId:    _tokenId,
            amount:     _amount
            });
        bytes memory pubData = Operations.writeDepositPubdata(op);
        addPriorityRequest(Operations.OpType.Deposit, pubData);

        emit OnchainDeposit(
            msg.sender,
            _tokenId,
            _amount,
            _owner
        );
    }

    /// @notice Register withdrawal - update user balance and emit OnchainWithdrawal event
    /// @param _token - token by id
    /// @param _amount - token amount
    /// @param _to - address to withdraw to
    function registerWithdrawal(uint16 _token, uint128 _amount, address payable _to) internal {
        bytes22 packedBalanceKey = packAddressAndTokenId(_to, _token);
        uint128 balance = balancesToWithdraw[packedBalanceKey].balanceToWithdraw;
        balancesToWithdraw[packedBalanceKey].balanceToWithdraw = balance.sub(_amount);
        emit OnchainWithdrawal(
            _to,
            _token,
            _amount
        );
    }

    /// @notice Store committed block structure to the storage.
    /// @param _nCommittedPriorityRequests - number of priority requests in block
    function createCommittedBlock(
        uint32 _blockNumber,
        uint32 _feeAccount,
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
            _newRoot // new root
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
                } else if (opType == Operations.OpType.ForcedExit) {
                    Operations.ForcedExit memory data = Operations.readForcedExitPubdata(_publicData, pubdataOffset + 1);

                    bool addToPendingWithdrawalsQueue = true;
                    withdrawalsDataHash = keccak256(abi.encode(withdrawalsDataHash, addToPendingWithdrawalsQueue, data.target, data.tokenId, data.amount));

                    pubDataPtr += FORCED_EXIT_BYTES;
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
    /// @param _oldRoot Old tree root
    /// @param _newRoot New tree root
    /// @param _publicData Operations pubdata
    /// @return block commitment
    function createBlockCommitment(
        uint32 _blockNumber,
        uint32 _feeAccount,
        bytes32 _oldRoot,
        bytes32 _newRoot,
        bytes memory _publicData
    ) internal view returns (bytes32 commitment) {
        bytes32 hash = sha256(
            abi.encodePacked(uint256(_blockNumber), uint256(_feeAccount))
        );
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

    /// @notice Checks that current state not is exodus mode
    function requireActive() internal view {
        require(!exodusMode, "fre11"); // exodus mode activated
    }

    // Priority queue

    /// @notice Saves priority request in storage
    /// @dev Calculates expiration block for request, store this request and emit NewPriorityRequest event
    /// @param _opType Rollup operation type
    /// @param _pubData Operation pubdata
    function addPriorityRequest(
        Operations.OpType _opType,
        bytes memory _pubData
    ) internal {
        // Expiration block is: current block number + priority expiration delta
        uint256 expirationBlock = block.number + PRIORITY_EXPIRATION;

        uint64 nextPriorityRequestId = firstPriorityRequestId + totalOpenPriorityRequests;

        priorityRequests[nextPriorityRequestId] = PriorityOperation({
            opType: _opType,
            pubData: _pubData,
            expirationBlock: expirationBlock
        });

        emit NewPriorityRequest(
            msg.sender,
            nextPriorityRequestId,
            _opType,
            _pubData,
            expirationBlock
        );

        totalOpenPriorityRequests++;
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

}
