// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

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

    bytes32 constant EMPTY_STRING_KECCAK = 0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470;

    /// @notice Data needed to process onchain operation from block public data.
    /// @notice Onchain operations is operations that need some processing on L1: Deposits, Withdrawals, ChangePubKey.
    /// @param publicDataOffset Byte offset in public data for onchain operation
    /// @param ethWitness Some external data that can be needed for operation processing
    struct OnchainOperationData {
        uint32 publicDataOffset;
        bytes ethWitness;
    }

    /// @notice Data needed to commit new block
    struct CommitBlockInfo {
        uint32 blockNumber;
        uint32 feeAccount;
        bytes32 newStateHash;
        bytes publicData;
        uint256 timestamp;
        OnchainOperationData[] onchainOperations;
    }

    /// @notice Data needed to execute committed and verified block
    /// @param commitmentsInSlot verified commitments in one slot
    /// @param commitmentIdx index such that commitmentsInSlot[commitmentIdx] is current block commitment
    struct ExecuteBlockInfo {
        StoredBlockInfo storedBlock;
        bytes[] pendingOnchainOpsPubdata;
    }

    /// @notice Recursive proof input data (individual commitments are constructed onchain)
    struct ProofInput {
        uint256[] recursiveInput;
        uint256[] proof;
        uint256[] commitments;
        uint8[] vkIndexes;
        uint256[16] subproofsLimbs;
    }

    // Upgrade functional

    /// @notice Notice period before activation preparation status of upgrade mode
    function getNoticePeriod() external pure override returns (uint256) {
        return UPGRADE_NOTICE_PERIOD;
    }

    /// @notice Notification that upgrade notice period started
    function upgradeNoticePeriodStarted() external override {}

    /// @notice Notification that upgrade preparation status is activated
    function upgradePreparationStarted() external override {
        upgradePreparationActive = true;
        upgradePreparationActivationTime = block.timestamp;
    }

    /// @notice Notification that upgrade canceled
    function upgradeCanceled() external override {
        upgradePreparationActive = false;
        upgradePreparationActivationTime = 0;
    }

    /// @notice Notification that upgrade finishes
    function upgradeFinishes() external override {
        upgradePreparationActive = false;
        upgradePreparationActivationTime = 0;
    }

    /// @notice Checks that contract is ready for upgrade
    /// @return bool flag indicating that contract is ready for upgrade
    function isReadyForUpgrade() external view override returns (bool) {
        return !exodusMode;
    }

    /// @notice zkSync contract initialization. Can be external because Proxy contract intercepts illegal calls of this function.
    /// @param initializationParameters Encoded representation of initialization parameters:
    /// @dev _governanceAddress The address of Governance contract
    /// @dev _verifierAddress The address of Verifier contract
    /// @dev _genesisStateHash Genesis blocks (first block) state tree root hash
    function initialize(bytes calldata initializationParameters) external {
        initializeReentrancyGuard();

        (address _governanceAddress, address _verifierAddress, bytes32 _genesisStateHash) =
            abi.decode(initializationParameters, (address, address, bytes32));

        verifier = Verifier(_verifierAddress);
        governance = Governance(_governanceAddress);

        // We need initial state hash because it is used in the commitment of the next block
        StoredBlockInfo memory storedBlockZero =
            StoredBlockInfo(0, 0, EMPTY_STRING_KECCAK, 0, _genesisStateHash, bytes32(0));

        storedBlockHashes[0] = hashStoredBlockInfo(storedBlockZero);
    }

    /// @notice zkSync contract upgrade. Can be external because Proxy contract intercepts illegal calls of this function.
    /// @param upgradeParameters Encoded representation of upgrade parameters
    function upgrade(bytes calldata upgradeParameters) external {
        require(upgradeParameters.length == 0, "af"); // upgrade parameters should be empty

        // Convert last verified block from old format to new format
        require(totalBlocksCommitted == totalBlocksVerified, "ag"); // all blocks should be verified
        require(numberOfPendingWithdrawals_DEPRECATED == 0, "ah"); // pending withdrawal is not used anymore
        require(totalOpenPriorityRequests == 0, "ai"); // no uncommitted priority requests

        Block_DEPRECATED memory lastBlock = blocks_DEPRECATED[totalBlocksVerified];
        require(lastBlock.priorityOperations == 0, "aj"); // last block should not contain priority operations

        StoredBlockInfo memory rehashedLastBlock =
            StoredBlockInfo(
                totalBlocksVerified,
                lastBlock.priorityOperations,
                EMPTY_STRING_KECCAK,
                0,
                lastBlock.stateRoot,
                lastBlock.commitment
            );
        storedBlockHashes[totalBlocksVerified] = hashStoredBlockInfo(rehashedLastBlock);
        totalBlocksProofed = totalBlocksVerified;
    }

    /// @notice Sends tokens
    /// @dev NOTE: will revert if transfer call fails or rollup balance difference (before and after transfer) is bigger than _maxAmount
    /// @param _tokenAddress Token address
    /// @param _to Address of recipient
    /// @param _amount Amount of tokens to transfer
    /// @param _maxAmount Maximum possible amount of tokens to transfer to this account
    function withdrawERC20Guarded(
        address _tokenAddress,
        address _to,
        uint128 _amount,
        uint128 _maxAmount
    ) external returns (uint128 withdrawnAmount) {
        require(msg.sender == address(this), "ak"); // wtg10 - can be called only from this contract as one "external" call (to revert all this function state changes if it is needed)
        IERC20 token = IERC20(_tokenAddress);

        uint256 balanceBefore = token.balanceOf(address(this));
        require(Utils.sendERC20(token, _to, _amount), "al"); // wtg11 - ERC20 transfer fails
        uint256 balanceAfter = token.balanceOf(address(this));
        uint256 balanceDiff = balanceBefore.sub(balanceAfter);
        require(balanceDiff <= _maxAmount, "am"); // wtg12 - rollup balance difference (before and after transfer) is bigger than _maxAmount

        return SafeCast.toUint128(balanceDiff);
    }

    /// @notice Accrues users balances from deposit priority requests in Exodus mode
    /// @dev WARNING: Only for Exodus mode
    /// @dev Canceling may take several separate transactions to be completed
    /// @param _n number of requests to process
    function cancelOutstandingDepositsForExodusMode(uint64 _n, bytes[] memory depositsPubdata) external nonReentrant {
        require(exodusMode, "an"); // exodus mode not active
        uint64 toProcess = Utils.minU64(totalOpenPriorityRequests, _n);
        require(toProcess > 0, "ao"); // no deposits to process
        uint64 currentDepositIdx = 0;
        for (uint64 id = firstPriorityRequestId; id < firstPriorityRequestId + toProcess; id++) {
            if (priorityRequests[id].opType == Operations.OpType.Deposit) {
                bytes memory depositPubdata = depositsPubdata[currentDepositIdx];
                require(Utils.hashBytesToBytes20(depositPubdata) == priorityRequests[id].hashedPubData, "ap");
                ++currentDepositIdx;

                Operations.Deposit memory op = Operations.readDepositPubdata(depositPubdata);
                bytes22 packedBalanceKey = packAddressAndTokenId(op.owner, op.tokenId);
                balancesToWithdraw[packedBalanceKey].balanceToWithdraw += op.amount;
            }
            delete priorityRequests[id];
        }
        firstPriorityRequestId += toProcess;
        totalOpenPriorityRequests -= toProcess;
    }

    /// @notice Withdraw ETH to Layer 1 - register withdrawal and transfer ether to sender
    /// @param _amount Ether amount to withdraw
    function withdrawETH(uint128 _amount) external nonReentrant {
        registerWithdrawal(0, _amount, msg.sender);
        (bool success, ) = msg.sender.call{value: _amount}("");
        require(success, "aq"); // ETH withdraw failed
    }

    /// @notice Deposit ETH to Layer 2 - transfer ether from user into contract, validate it, register deposit
    /// @param _zkSyncAddress The receiver Layer 2 address
    function depositETH(address _zkSyncAddress) external payable nonReentrant {
        requireActive();
        registerDeposit(0, SafeCast.toUint128(msg.value), _zkSyncAddress);
    }

    /// @notice Deposit ERC20 token to Layer 2 - transfer ERC20 tokens from user into contract, validate it, register deposit
    /// @param _token Token address
    /// @param _amount Token amount
    /// @param _zkSyncAddress Receiver Layer 2 address
    function depositERC20(
        IERC20 _token,
        uint104 _amount,
        address _zkSyncAddress
    ) external nonReentrant {
        requireActive();

        // Get token id by its address
        uint16 tokenId = governance.validateTokenAddress(address(_token));
        require(!governance.pausedTokens(tokenId), "ar"); // token deposits are paused

        uint256 balanceBefore = _token.balanceOf(address(this));
        require(Utils.transferFromERC20(_token, msg.sender, address(this), SafeCast.toUint128(_amount)), "as"); // token transfer failed deposit
        uint256 balanceAfter = _token.balanceOf(address(this));
        uint128 depositAmount = SafeCast.toUint128(balanceAfter.sub(balanceBefore));

        registerDeposit(tokenId, depositAmount, _zkSyncAddress);
    }

    /// @notice Withdraw ERC20 token to Layer 1 - register withdrawal and transfer ERC20 to sender
    /// @param _token Token address
    /// @param _amount amount to withdraw
    function withdrawERC20(address _token, uint128 _amount) external nonReentrant {
        uint16 tokenId = governance.validateTokenAddress(_token);
        bytes22 packedBalanceKey = packAddressAndTokenId(msg.sender, tokenId);
        uint128 balance = balancesToWithdraw[packedBalanceKey].balanceToWithdraw;
        uint128 withdrawnAmount = this.withdrawERC20Guarded(_token, msg.sender, _amount, balance);
        registerWithdrawal(tokenId, withdrawnAmount, msg.sender);
    }

    /// @notice Register full exit request - pack pubdata, add priority request
    /// @param _accountId Numerical id of the account
    /// @param _token Token address, 0 address for ether
    function fullExit(uint32 _accountId, address _token) external nonReentrant {
        requireActive();
        require(_accountId <= MAX_ACCOUNT_ID, "at");

        uint16 tokenId;
        if (_token == address(0)) {
            tokenId = 0;
        } else {
            tokenId = governance.validateTokenAddress(_token);
        }

        // Priority Queue request
        Operations.FullExit memory op =
            Operations.FullExit({
                accountId: _accountId,
                owner: msg.sender,
                tokenId: tokenId,
                amount: 0 // unknown at this point
            });
        bytes memory pubData = Operations.writeFullExitPubdata(op);
        addPriorityRequest(Operations.OpType.FullExit, pubData);

        // User must fill storage slot of balancesToWithdraw(msg.sender, tokenId) with nonzero value
        // In this case operator should just overwrite this slot during confirming withdrawal
        bytes22 packedBalanceKey = packAddressAndTokenId(msg.sender, tokenId);
        balancesToWithdraw[packedBalanceKey].gasReserveValue = FILLED_GAS_RESERVE_VALUE;
    }

    /// @dev Process one block commit using previous block StoredBlockInfo,
    /// @dev returns new block StoredBlockInfo
    /// @dev NOTE: Does not change storage (except events, so we can't mark it view)
    function commitOneBlock(StoredBlockInfo memory _previousBlock, CommitBlockInfo memory _newBlock)
        internal
        returns (StoredBlockInfo memory storedNewBlock)
    {
        require(_newBlock.blockNumber == _previousBlock.blockNumber + 1, "au"); // only commit next block

        // Check timestamp of the new block
        {
            require(_newBlock.timestamp >= _previousBlock.timestamp, "av"); // Block should be after previous block
            bool timestampNotTooSmall = block.timestamp - COMMIT_TIMESTAMP_NOT_OLDER <= _newBlock.timestamp;
            bool timestampNotTooBig = _newBlock.timestamp <= block.timestamp + COMMIT_TIMESTAMP_APPROXIMATION_DELTA;
            require(timestampNotTooSmall && timestampNotTooBig, "aw"); // New block timestamp is not valid
        }

        // Check onchain operations
        (bytes32 pendingOnchainOpsHash, uint64 priorityReqCommitted, bytes memory onchainOpsOffsetCommitment) =
            collectOnchainOps(_newBlock);

        // Create block commitment for verification proof
        bytes32 commitment = createBlockCommitment(_previousBlock, _newBlock, onchainOpsOffsetCommitment);

        return
            StoredBlockInfo(
                _newBlock.blockNumber,
                priorityReqCommitted,
                pendingOnchainOpsHash,
                _newBlock.timestamp,
                _newBlock.newStateHash,
                commitment
            );
    }

    /// @notice Commit block
    /// @notice 1. Checks onchain operations, timestamp.
    /// @notice 2. Store block commitments
    function commitBlocks(StoredBlockInfo memory _lastCommittedBlockData, CommitBlockInfo[] memory _newBlocksData)
        external
        nonReentrant
    {
        requireActive();
        governance.requireActiveValidator(msg.sender);
        // Check that we commit blocks after last committed block
        require(storedBlockHashes[totalBlocksCommitted] == hashStoredBlockInfo(_lastCommittedBlockData), "ax"); // incorrect previous block data

        StoredBlockInfo memory lastCommittedBlock = _lastCommittedBlockData;

        uint64 committedPriorityRequests = 0;
        for (uint32 i = 0; i < _newBlocksData.length; ++i) {
            lastCommittedBlock = commitOneBlock(lastCommittedBlock, _newBlocksData[i]);

            committedPriorityRequests += lastCommittedBlock.priorityOperations;
            storedBlockHashes[lastCommittedBlock.blockNumber] = hashStoredBlockInfo(lastCommittedBlock);

            emit BlockCommit(lastCommittedBlock.blockNumber);
        }

        totalBlocksCommitted += uint32(_newBlocksData.length);

        totalCommittedPriorityRequests += committedPriorityRequests;
        require(totalCommittedPriorityRequests <= totalOpenPriorityRequests, "ay");
    }

    /// @dev 1. Try to send token to _recipients
    /// @dev 2. On failure: Increment _recipients balance to withdraw.
    function withdrawOrStore(
        uint16 _tokenId,
        address _recipient,
        uint128 _amount
    ) internal {
        emit RollupWithdrawal(_recipient, _tokenId, _amount);
        bytes22 packedBalanceKey = packAddressAndTokenId(_recipient, _tokenId);

        bool sent = false;
        if (_tokenId == 0) {
            address payable toPayable = address(uint160(_recipient));
            sent = Utils.sendETHNoRevert(toPayable, _amount);
        } else {
            address tokenAddr = governance.tokenAddresses(_tokenId);
            try this.withdrawERC20Guarded{gas: ERC20_WITHDRAWAL_GAS_LIMIT}(tokenAddr, _recipient, _amount, _amount) {
                sent = true;
            } catch {
                sent = false;
            }
        }
        if (sent) {
            emit OnchainWithdrawal(_recipient, _tokenId, _amount);
        } else {
            increaseBalanceToWithdraw(packedBalanceKey, _amount);
        }
    }

    /// @dev Executes one block
    /// @dev 1. Processes all pending operations (Send Exits, Complete priority requests)
    /// @dev 2. Finalizes block on Ethereum
    /// @dev _executedBlockIdx is index in the array of the blocks that we want to execute together
    function executeOneBlock(ExecuteBlockInfo memory _blockExecuteData, uint32 _executedBlockIdx) internal {
        // Ensure block was committed
        require(
            hashStoredBlockInfo(_blockExecuteData.storedBlock) ==
                storedBlockHashes[_blockExecuteData.storedBlock.blockNumber],
            "exe10" // executing block should be committed
        );
        require(_blockExecuteData.storedBlock.blockNumber == totalBlocksVerified + _executedBlockIdx + 1, "az"); // Execute blocks in order
        require(_blockExecuteData.storedBlock.blockNumber <= totalBlocksProofed, "ba"); // Can't execute blocks more then committed and proofed currently.

        bytes32 pendingOnchainOpsHash = EMPTY_STRING_KECCAK;
        for (uint32 i = 0; i < _blockExecuteData.pendingOnchainOpsPubdata.length; ++i) {
            bytes memory pubData = _blockExecuteData.pendingOnchainOpsPubdata[i];

            Operations.OpType opType = Operations.OpType(uint8(pubData[0]));

            if (opType == Operations.OpType.PartialExit) {
                Operations.PartialExit memory op = Operations.readPartialExitPubdata(pubData);
                withdrawOrStore(op.tokenId, op.owner, op.amount);
            } else if (opType == Operations.OpType.ForcedExit) {
                Operations.ForcedExit memory op = Operations.readForcedExitPubdata(pubData);
                withdrawOrStore(op.tokenId, op.target, op.amount);
            } else if (opType == Operations.OpType.FullExit) {
                Operations.FullExit memory op = Operations.readFullExitPubdata(pubData);
                withdrawOrStore(op.tokenId, op.owner, op.amount);
            } else {
                revert("exe13"); // unsupported op in block execution
            }

            pendingOnchainOpsHash = Utils.concatHash(pendingOnchainOpsHash, pubData);
        }
        require(pendingOnchainOpsHash == _blockExecuteData.storedBlock.pendingOnchainOperationsHash, "bb"); // incorrect onchain ops executed
    }

    /// @notice Execute blocks, completing priority operations and processing withdrawals.
    /// @notice 1. Processes all pending operations (Send Exits, Complete priority requests)
    /// @notice 2. Finalizes block on Ethereum
    function executeBlocks(ExecuteBlockInfo[] memory _blocksData) external nonReentrant {
        requireActive();
        governance.requireActiveValidator(msg.sender);

        uint64 priorityRequestsExecuted = 0;
        uint32 nBlocks = uint32(_blocksData.length);
        for (uint32 i = 0; i < nBlocks; ++i) {
            executeOneBlock(_blocksData[i], i);
            priorityRequestsExecuted += _blocksData[i].storedBlock.priorityOperations;
            emit BlockVerification(_blocksData[i].storedBlock.blockNumber);
        }

        firstPriorityRequestId += priorityRequestsExecuted;
        totalCommittedPriorityRequests -= priorityRequestsExecuted;
        totalOpenPriorityRequests -= priorityRequestsExecuted;

        totalBlocksVerified += nBlocks;
    }

    /// @notice Blocks commitment verification.
    /// @notice Only verifies block commitments without any other processing
    function proofBlocks(StoredBlockInfo[] memory _committedBlocks, ProofInput memory _proof) external nonReentrant {
        uint32 currentTotalBlocksProofed = totalBlocksProofed;
        for (uint256 i = 0; i < _committedBlocks.length; ++i) {
            require(
                hashStoredBlockInfo(_committedBlocks[i]) == storedBlockHashes[currentTotalBlocksProofed + 1],
                "pbl2"
            );
            ++currentTotalBlocksProofed;

            uint256 mask = (~uint256(0)) >> 3;
            require(_proof.commitments[i] & mask == uint256(_committedBlocks[i].commitment) & mask, "bc"); // incorrect block commitment in proof
        }

        bool success =
            verifier.verifyAggregatedProof(
                _proof.recursiveInput,
                _proof.proof,
                _proof.vkIndexes,
                _proof.commitments,
                _proof.subproofsLimbs,
                true
            );
        require(success, "bd"); // Aggregated proof verification fail

        require(currentTotalBlocksProofed <= totalBlocksCommitted, "be");
        totalBlocksProofed = currentTotalBlocksProofed;
    }

    /// @notice Reverts unverified blocks
    function revertBlocks(StoredBlockInfo[] memory _blocksToRevert) external nonReentrant {
        governance.requireActiveValidator(msg.sender);

        uint32 blocksCommitted = totalBlocksCommitted;
        uint32 blocksToRevert = Utils.minU32(uint32(_blocksToRevert.length), blocksCommitted - totalBlocksVerified);
        uint64 revertedPriorityRequests = 0;

        for (uint32 i = 0; i < blocksToRevert; ++i) {
            StoredBlockInfo memory storedBlockInfo = _blocksToRevert[i];
            require(storedBlockHashes[blocksCommitted] == hashStoredBlockInfo(storedBlockInfo), "bf"); // incorrect stored block info

            delete storedBlockHashes[blocksCommitted];

            --blocksCommitted;
            revertedPriorityRequests += storedBlockInfo.priorityOperations;
        }

        totalBlocksCommitted = blocksCommitted;
        totalCommittedPriorityRequests -= revertedPriorityRequests;
        if (totalBlocksCommitted < totalBlocksProofed) {
            totalBlocksProofed = totalBlocksCommitted;
        }

        emit BlocksRevert(totalBlocksVerified, blocksCommitted);
    }

    /// @notice Checks if Exodus mode must be entered. If true - enters exodus mode and emits ExodusMode event.
    /// @dev Exodus mode must be entered in case of current ethereum block number is higher than the oldest
    /// @dev of existed priority requests expiration block number.
    /// @return bool flag that is true if the Exodus mode must be entered.
    function triggerExodusIfNeeded() external returns (bool) {
        bool trigger =
            block.number >= priorityRequests[firstPriorityRequestId].expirationBlock &&
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
    /// @param _storedBlockInfo Last verified block
    /// @param _accountId Id of the account in the tree
    /// @param _proof Proof
    /// @param _tokenId Verified token id
    /// @param _amount Amount for owner (must be total amount, not part of it)
    function exit(
        StoredBlockInfo memory _storedBlockInfo,
        uint32 _accountId,
        uint16 _tokenId,
        uint128 _amount,
        ProofInput memory _proof
    ) external nonReentrant {
        bytes22 packedBalanceKey = packAddressAndTokenId(msg.sender, _tokenId);
        require(exodusMode, "bg"); // must be in exodus mode
        require(!exited[_accountId][_tokenId], "bh"); // already exited
        require(storedBlockHashes[totalBlocksVerified] == hashStoredBlockInfo(_storedBlockInfo), "bi"); // incorrect sotred block info

        uint256 commitment =
            uint256(sha256(abi.encodePacked(_storedBlockInfo.stateHash, _accountId, msg.sender, _tokenId, _amount)));
        require(_proof.commitments.length == 1, "bj");
        uint256 mask = (~uint256(0)) >> 3;
        commitment = commitment & mask;
        require(_proof.commitments[0] == commitment, "bk");

        bool proofCorrect =
            verifier.verifyAggregatedProof(
                _proof.recursiveInput,
                _proof.proof,
                _proof.vkIndexes,
                _proof.commitments,
                _proof.subproofsLimbs,
                false
            );
        require(proofCorrect, "bl");

        increaseBalanceToWithdraw(packedBalanceKey, _amount);
        exited[_accountId][_tokenId] = true;
    }

    function setAuthPubkeyHash(bytes calldata _pubkey_hash, uint32 _nonce) external nonReentrant {
        require(_pubkey_hash.length == PUBKEY_HASH_BYTES, "bm"); // PubKeyHash should be 20 bytes.
        require(authFacts[msg.sender][_nonce] == bytes32(0), "bn"); // auth fact for nonce should be empty

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
        Operations.Deposit memory op =
            Operations.Deposit({
                accountId: 0, // unknown at this point
                owner: _owner,
                tokenId: _tokenId,
                amount: _amount
            });
        bytes memory pubData = Operations.writeDepositPubdata(op);
        addPriorityRequest(Operations.OpType.Deposit, pubData);

        emit OnchainDeposit(msg.sender, _tokenId, _amount, _owner);
    }

    /// @notice Register withdrawal - update user balance and emit OnchainWithdrawal event
    /// @param _token - token by id
    /// @param _amount - token amount
    /// @param _to - address to withdraw to
    function registerWithdrawal(
        uint16 _token,
        uint128 _amount,
        address payable _to
    ) internal {
        bytes22 packedBalanceKey = packAddressAndTokenId(_to, _token);
        uint128 balance = balancesToWithdraw[packedBalanceKey].balanceToWithdraw;
        balancesToWithdraw[packedBalanceKey].balanceToWithdraw = balance.sub(_amount);
        emit OnchainWithdrawal(_to, _token, _amount);
    }

    function emitDepositCommitEvent(uint32 _blockNumber, Operations.Deposit memory depositData) internal {
        emit DepositCommit(
            _blockNumber,
            depositData.accountId,
            depositData.owner,
            depositData.tokenId,
            depositData.amount
        );
    }

    function emitFullExitCommitEvent(uint32 _blockNumber, Operations.FullExit memory fullExitData) internal {
        emit FullExitCommit(
            _blockNumber,
            fullExitData.accountId,
            fullExitData.owner,
            fullExitData.tokenId,
            fullExitData.amount
        );
    }

    /// @dev Gets operations packed in bytes array. Unpacks it and stores onchain operations.
    /// @dev Priority operations must be committed in the same order as they are in the priority queue.
    /// @dev NOTE: does not change storage! (only emits events)
    /// @dev processableOperationsHash - hash of the all operations that needs to be executed  (Deposit, Exits, ChangPubKey)
    /// @dev priorityOperationsProcessed - number of priority operations processed in this block (Deposits, FullExits)
    /// @dev offsetsCommitment - array where 1 is stored in chunk where onchainOperation begins and other are 0 (used in commitments)
    function collectOnchainOps(CommitBlockInfo memory _newBlockData)
        internal
        returns (
            bytes32 processableOperationsHash,
            uint64 priorityOperationsProcessed,
            bytes memory offsetsCommitment
        )
    {
        bytes memory pubData = _newBlockData.publicData;

        uint64 uncommittedPriorityRequestsOffset = firstPriorityRequestId + totalCommittedPriorityRequests;
        priorityOperationsProcessed = 0;
        processableOperationsHash = EMPTY_STRING_KECCAK;

        require(pubData.length % CHUNK_BYTES == 0, "bo"); // pubdata length must be a multiple of CHUNK_BYTES
        offsetsCommitment = new bytes(pubData.length / CHUNK_BYTES);
        for (uint32 i = 0; i < _newBlockData.onchainOperations.length; ++i) {
            OnchainOperationData memory onchainOpData = _newBlockData.onchainOperations[i];

            uint256 pubdataOffset = onchainOpData.publicDataOffset;
            require(pubdataOffset % CHUNK_BYTES == 0, "bp"); // offsets should be on chunks boundaries
            require(offsetsCommitment[pubdataOffset / CHUNK_BYTES] == 0x00, "bq"); // offset commitment should be empty
            offsetsCommitment[pubdataOffset / CHUNK_BYTES] = bytes1(0x01);

            Operations.OpType opType = Operations.OpType(uint8(pubData[pubdataOffset]));

            if (opType == Operations.OpType.Deposit) {
                bytes memory opPubData = Bytes.slice(pubData, pubdataOffset, DEPOSIT_BYTES);

                Operations.Deposit memory depositData = Operations.readDepositPubdata(opPubData);
                emitDepositCommitEvent(_newBlockData.blockNumber, depositData);

                checkPriorityOperation(depositData, uncommittedPriorityRequestsOffset + priorityOperationsProcessed);
                priorityOperationsProcessed++;
            } else if (opType == Operations.OpType.PartialExit) {
                bytes memory opPubData = Bytes.slice(pubData, pubdataOffset, PARTIAL_EXIT_BYTES);

                processableOperationsHash = Utils.concatHash(processableOperationsHash, opPubData);
            } else if (opType == Operations.OpType.ForcedExit) {
                bytes memory opPubData = Bytes.slice(pubData, pubdataOffset, FORCED_EXIT_BYTES);

                processableOperationsHash = Utils.concatHash(processableOperationsHash, opPubData);
            } else if (opType == Operations.OpType.FullExit) {
                bytes memory opPubData = Bytes.slice(pubData, pubdataOffset, FULL_EXIT_BYTES);

                Operations.FullExit memory fullExitData = Operations.readFullExitPubdata(opPubData);
                emitFullExitCommitEvent(_newBlockData.blockNumber, fullExitData);

                checkPriorityOperation(fullExitData, uncommittedPriorityRequestsOffset + priorityOperationsProcessed);
                priorityOperationsProcessed++;

                processableOperationsHash = Utils.concatHash(processableOperationsHash, opPubData);
            } else if (opType == Operations.OpType.ChangePubKey) {
                bytes memory opPubData = Bytes.slice(pubData, pubdataOffset, CHANGE_PUBKEY_BYTES);

                Operations.ChangePubKey memory op = Operations.readChangePubKeyPubdata(opPubData);

                if (onchainOpData.ethWitness.length != 0) {
                    bool valid = verifyChangePubkey(onchainOpData.ethWitness, op);
                    require(valid, "br"); // failed to verify change pubkey hash signature
                } else {
                    bool valid = authFacts[op.owner][op.nonce] == keccak256(abi.encodePacked(op.pubKeyHash));
                    require(valid, "bs"); // new pub key hash is not authenticated properly
                }
            } else {
                revert("fpp14"); // unsupported op
            }
        }
    }

    /// @notice Checks that change operation is correct
    function verifyChangePubkey(bytes memory _ethWitness, Operations.ChangePubKey memory _changePk)
        internal
        pure
        returns (bool)
    {
        Operations.ChangePubkeyType changePkType = Operations.ChangePubkeyType(uint8(_ethWitness[0]));
        if (changePkType == Operations.ChangePubkeyType.ECRECOVER) {
            return verifyChangePubkeyECRECOVER(_ethWitness, _changePk);
        } else if (changePkType == Operations.ChangePubkeyType.CREATE2) {
            return verifyChangePubkeyCREATE2(_ethWitness, _changePk);
        } else {
            revert("chp13"); // Incorrect ChangePubKey type
        }
    }

    /// @notice Checks that signature is valid for pubkey change message
    /// @param _ethWitness Signature (65 bytes) + 32 bytes of the arbitrary signed data
    /// @param _changePk Parsed change pubkey operation
    function verifyChangePubkeyECRECOVER(bytes memory _ethWitness, Operations.ChangePubKey memory _changePk)
        internal
        pure
        returns (bool)
    {
        (uint256 offset, bytes memory signature) = Bytes.read(_ethWitness, 1, 65); // offset is 1 because we skip type of ChangePubkey
        (, bytes32 additionalData) = Bytes.readBytes32(_ethWitness, offset);
        bytes32 messageHash =
            keccak256(
                abi.encodePacked(
                    "\x19Ethereum Signed Message:\n60",
                    _changePk.pubKeyHash,
                    _changePk.nonce,
                    _changePk.accountId,
                    additionalData
                )
            );
        address recoveredAddress = Utils.recoverAddressFromEthSignature(signature, messageHash);
        return recoveredAddress == _changePk.owner;
    }

    /// @notice Checks that signature is valid for pubkey change message
    /// @param _ethWitness Create2 deployer address, saltArg, codeHash
    /// @param _changePk Parsed change pubkey operation
    function verifyChangePubkeyCREATE2(bytes memory _ethWitness, Operations.ChangePubKey memory _changePk)
        internal
        pure
        returns (bool)
    {
        address creatorAddress;
        bytes32 saltArg; // salt arg is additional bytes that are encoded in the CREATE2 salt
        bytes32 codeHash;
        uint256 offset = 1; // offset is 1 because we skip type of ChangePubkey
        (offset, creatorAddress) = Bytes.readAddress(_ethWitness, offset);
        (offset, saltArg) = Bytes.readBytes32(_ethWitness, offset);
        (offset, codeHash) = Bytes.readBytes32(_ethWitness, offset);
        // salt from CREATE2 specification
        bytes32 salt = keccak256(abi.encodePacked(_changePk.pubKeyHash, saltArg));
        // Address computation according to CREATE2 definition: https://eips.ethereum.org/EIPS/eip-1014
        address recoveredAddress =
            address(uint160(uint256(keccak256(abi.encodePacked(bytes1(0xff), creatorAddress, salt, codeHash)))));
        // This type of change pubkey can be done only once
        return recoveredAddress == _changePk.owner && _changePk.nonce == 0;
    }

    /// @dev Creates block commitment from its data
    /// @dev _offsetCommitment - hash of the array where 1 is stored in chunk where onchainOperation begins and 0 for other chunks
    function createBlockCommitment(
        StoredBlockInfo memory _previousBlock,
        CommitBlockInfo memory _newBlockData,
        bytes memory _offsetCommitment
    ) internal view returns (bytes32 commitment) {
        bytes32 hash = sha256(abi.encodePacked(uint256(_newBlockData.blockNumber), uint256(_newBlockData.feeAccount)));
        hash = sha256(abi.encodePacked(hash, _previousBlock.stateHash));
        hash = sha256(abi.encodePacked(hash, _newBlockData.newStateHash));
        hash = sha256(abi.encodePacked(hash, uint256(_newBlockData.timestamp)));

        bytes memory pubdata = abi.encodePacked(_newBlockData.publicData, _offsetCommitment);

        /// The code below is equivalent to `commitment = sha256(abi.encodePacked(hash, _publicData))`

        /// We use inline assembly instead of this concise and readable code in order to avoid copying of `_publicData` (which saves ~90 gas per transfer operation).

        /// Specifically, we perform the following trick:
        /// First, replace the first 32 bytes of `_publicData` (where normally its length is stored) with the value of `hash`.
        /// Then, we call `sha256` precompile passing the `_publicData` pointer and the length of the concatenated byte buffer.
        /// Finally, we put the `_publicData.length` back to its original location (to the first word of `_publicData`).
        assembly {
            let hashResult := mload(0x40)
            let pubDataLen := mload(pubdata)
            mstore(pubdata, hash)
            // staticcall to the sha256 precompile at address 0x2
            let success := staticcall(gas(), 0x2, pubdata, add(pubDataLen, 0x20), hashResult, 0x20)
            mstore(pubdata, pubDataLen)

            // Use "invalid" to make gas estimation work
            switch success
                case 0 {
                    invalid()
                }

            commitment := mload(hashResult)
        }
    }

    /// @notice Checks that deposit is same as operation in priority queue
    /// @param _deposit Deposit data
    /// @param _priorityRequestId Operation's id in priority queue
    function checkPriorityOperation(Operations.Deposit memory _deposit, uint64 _priorityRequestId) internal view {
        Operations.OpType priorReqType = priorityRequests[_priorityRequestId].opType;
        require(priorReqType == Operations.OpType.Deposit, "bt"); // incorrect priority op type

        bytes20 hashedPubdata = priorityRequests[_priorityRequestId].hashedPubData;
        require(Operations.checkDepositInPriorityQueue(_deposit, hashedPubdata), "bu");
    }

    /// @notice Checks that FullExit is same as operation in priority queue
    /// @param _fullExit FullExit data
    /// @param _priorityRequestId Operation's id in priority queue
    function checkPriorityOperation(Operations.FullExit memory _fullExit, uint64 _priorityRequestId) internal view {
        Operations.OpType priorReqType = priorityRequests[_priorityRequestId].opType;
        require(priorReqType == Operations.OpType.FullExit, "bv"); // incorrect priority op type

        bytes20 hashedPubdata = priorityRequests[_priorityRequestId].hashedPubData;
        require(Operations.checkFullExitInPriorityQueue(_fullExit, hashedPubdata), "bw");
    }

    /// @notice Checks that current state not is exodus mode
    function requireActive() internal view {
        require(!exodusMode, "bx"); // exodus mode activated
    }

    // Priority queue

    /// @notice Saves priority request in storage
    /// @dev Calculates expiration block for request, store this request and emit NewPriorityRequest event
    /// @param _opType Rollup operation type
    /// @param _pubData Operation pubdata
    function addPriorityRequest(Operations.OpType _opType, bytes memory _pubData) internal {
        // Expiration block is: current block number + priority expiration delta
        uint64 expirationBlock = uint64(block.number + PRIORITY_EXPIRATION);

        uint64 nextPriorityRequestId = firstPriorityRequestId + totalOpenPriorityRequests;

        bytes20 hashedPubData = Utils.hashBytesToBytes20(_pubData);

        priorityRequests[nextPriorityRequestId] = PriorityOperation({
            hashedPubData: hashedPubData,
            expirationBlock: expirationBlock,
            opType: _opType
        });

        emit NewPriorityRequest(msg.sender, nextPriorityRequestId, _opType, _pubData, uint256(expirationBlock));

        totalOpenPriorityRequests++;
    }

    /// @notice Deletes processed priority requests
    /// @param _number The number of requests
    function deleteRequests(uint64 _number) internal {
        require(_number <= totalOpenPriorityRequests, "by"); // number is higher than total priority requests number

        uint64 numberOfRequestsToClear = Utils.minU64(_number, MAX_PRIORITY_REQUESTS_TO_DELETE_IN_VERIFY);
        uint64 startIndex = firstPriorityRequestId;
        for (uint64 i = startIndex; i < startIndex + numberOfRequestsToClear; i++) {
            delete priorityRequests[i];
        }

        totalOpenPriorityRequests -= _number;
        firstPriorityRequestId += _number;
        totalCommittedPriorityRequests -= _number;
    }

    function increaseBalanceToWithdraw(bytes22 _packedBalanceKey, uint128 _amount) internal {
        uint128 balance = balancesToWithdraw[_packedBalanceKey].balanceToWithdraw;
        balancesToWithdraw[_packedBalanceKey] = BalanceToWithdraw(balance.add(_amount), FILLED_GAS_RESERVE_VALUE);
    }
}
