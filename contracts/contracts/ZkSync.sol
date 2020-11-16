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
import "hardhat/console.sol";

/// @title zkSync main contract
/// @author Matter Labs
contract ZkSync is UpgradeableMaster, Storage, Config, Events, ReentrancyGuard {
    using SafeMath for uint256;
    using SafeMathUInt128 for uint128;

    bytes32 public constant EMPTY_STRING_KECCAK = 0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470;

    // Upgrade functional

    /// @notice Notice period before activation preparation status of upgrade mode
    function getNoticePeriod() external pure override returns (uint) {
        return UPGRADE_NOTICE_PERIOD;
    }

    /// @notice Notification that upgrade notice period started
    function upgradeNoticePeriodStarted() external override {

    }

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

    /// @notice Franklin contract initialization. Can be external because Proxy contract intercepts illegal calls of this function.
    /// @param initializationParameters Encoded representation of initialization parameters:
    /// _governanceAddress The address of Governance contract
    /// _verifierAddress The address of Verifier contract
    /// _genesisRoot Genesis blocks (first block) root
    function initialize(bytes calldata initializationParameters) external {
        initializeReentrancyGuard();

        (
        address _governanceAddress,
        address _verifierAddress,
        bytes32 _blockRootHash
        ) = abi.decode(initializationParameters, (address, address, bytes32));

        verifier = Verifier(_verifierAddress);
        governance = Governance(_governanceAddress);

        StoredBlockInfo memory storedBlockZero = StoredBlockInfo(0, 0, EMPTY_STRING_KECCAK, 0, _blockRootHash, bytes32(0));

        hashedBlocks[0] = hashStoredBlockInfo(storedBlockZero);
    }

    /// @notice zkSync contract upgrade. Can be external because Proxy contract intercepts illegal calls of this function.
    /// @param upgradeParameters Encoded representation of upgrade parameters
    function upgrade(bytes calldata upgradeParameters) external {
        // rehash last verified block to support new schema
        require(totalBlocksCommitted == totalBlocksVerified, "upg1"); // all blocks should be verified
        require(upgradeParameters.length == 0, "upg3"); // upgrade parameters should be empty

        Block memory lastBlock = blocks[totalBlocksVerified];
        require(lastBlock.priorityOperations == 0, "upg2"); // last block should not contain priority operations

        StoredBlockInfo memory rehashedLastBlock = StoredBlockInfo(totalBlocksVerified, lastBlock.priorityOperations, EMPTY_STRING_KECCAK, 0, lastBlock.stateRoot, lastBlock.commitment);
        hashedBlocks[totalBlocksVerified] = hashStoredBlockInfo(rehashedLastBlock);
    }

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
        (bool success, ) = msg.sender.call{ value: _amount }("");
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

    /// @notice Data needed to process onchain operation from block public data
    struct OnchainOperationData {
        uint32 publicDataOffset;
        bytes ethWitness;
    }

    /// @notice Data needed to commit new block
    struct CommitBlockInfo {
        uint32 blockNumber;
        uint32 feeAccount;
        bytes32 newStateRoot;
        bytes publicData;
        uint256 timestamp;
        OnchainOperationData[] onchainOperations;
    }

    /// @dev Process one block commit using previous block StoredBlockInfo, returns new block StoredBlockInfo
    function commitOneBlock(StoredBlockInfo memory _previousBlock, CommitBlockInfo memory _newBlock)
      internal returns (StoredBlockInfo memory storedNewBlock) {
        require(_newBlock.blockNumber == _previousBlock.blockNumber + 1, "fck11"); // only commit next block

        require(_newBlock.timestamp >= _previousBlock.timestamp, "tms11"); // Block should be after previous block
        require((block.timestamp - COMMIT_TIMESTAMP_NOT_OLDER) <= _newBlock.timestamp
            && _newBlock.timestamp <= (block.timestamp + COMMIT_TIMESTAMP_APPROXIMATION_DELTA), "tms12"); // tms12 - _blockTimestamp is not valid

        (bytes32 processedOnchainOpsHash, uint64 priorityRequests) = collectOnchainOps(_newBlock);

        // Create block commitment for verification proof
        bytes32 commitment = createBlockCommitment(_previousBlock, _newBlock);

        return StoredBlockInfo(_newBlock.blockNumber, priorityRequests, processedOnchainOpsHash, _newBlock.timestamp, _newBlock.newStateRoot, commitment);
    }

    /// @notice Commit block - collect onchain operations, create its commitment, emit BlockCommit event
    function commitBlocks(
        StoredBlockInfo memory _lastCommittedBlockData,
        CommitBlockInfo[] memory _newBlocksData
    ) external nonReentrant {
        requireActive();
        governance.requireActiveValidator(msg.sender);
        require(hashedBlocks[_lastCommittedBlockData.blockNumber] == hashStoredBlockInfo(_lastCommittedBlockData), "fck10"); // incorrect previous block data

        StoredBlockInfo memory lastCommittedBlock = _lastCommittedBlockData;
        for (uint32 i = 0; i < _newBlocksData.length; ++i) {
            lastCommittedBlock = commitOneBlock(lastCommittedBlock, _newBlocksData[i]);

            totalCommittedPriorityRequests += lastCommittedBlock.priorityOperations;
            hashedBlocks[lastCommittedBlock.blockNumber] = hashStoredBlockInfo(lastCommittedBlock);

            emit BlockCommit(lastCommittedBlock.blockNumber);
        }

        totalBlocksCommitted += uint32(_newBlocksData.length);
    }

    /// @notice Data needed to execute committed and verified block
    struct ExecuteBlockInfo {
        StoredBlockInfo storedBlock;
        bytes[] onchainOpsPubdata;
        bytes32[] commitmentsInSlot;
        uint256 commitmentIdx;
    }

    /// @dev Try withdrawing token and if failure - add to the onchain balance.
    function withdrawOrStore(uint16 _tokenId, address _recipient, uint128 _amount) internal {
        bytes22 packedBalanceKey = packAddressAndTokenId(_recipient, _tokenId);

        bool sent = false;
        if (_tokenId == 0) {
            address payable toPayable = address(uint160(_recipient));
            sent = Utils.sendETHNoRevert(toPayable, _amount);
        } else {
            address tokenAddr = governance.tokenAddresses(_tokenId);
            // we can just check that call not reverts because it wants to withdraw all amount
            try this.withdrawERC20Guarded{gas: ERC20_WITHDRAWAL_GAS_LIMIT}(IERC20(tokenAddr), _recipient, _amount, _amount) {
                sent = true;
            } catch {
                sent = false;
            }
        }
        if (!sent) {
            incBalanceToWithdraw(packedBalanceKey, _amount);
        }
    }

    /// @notice Executes one block
    function executeOneBlock(
        ExecuteBlockInfo memory _blockExecuteData,
        uint32 executedBlockIdx
    ) internal returns (uint32 priorityRequestsExecuted) {
        // ensure block was committed
        require(hashStoredBlockInfo(_blockExecuteData.storedBlock) == hashedBlocks[_blockExecuteData.storedBlock.blockNumber], "exe10"); // incorrect previous block data
        require(_blockExecuteData.storedBlock.blockNumber == totalBlocksVerified + executedBlockIdx + 1, "exe11"); // Execute blocks in order
        // ensure block was verified
        require(openAndCheckCommitmentInSlot(_blockExecuteData.storedBlock.commitment, _blockExecuteData.commitmentsInSlot, _blockExecuteData.commitmentIdx), "exe12"); // block is verified

        priorityRequestsExecuted = 0;
        bytes32 processableOnchainOpsHash = EMPTY_STRING_KECCAK;
        for (uint32 i = 0; i < _blockExecuteData.onchainOpsPubdata.length; ++i) {
            bytes memory pubData = _blockExecuteData.onchainOpsPubdata[i];

            Operations.OpType opType = Operations.OpType(uint8(pubData[0]));

            if (opType == Operations.OpType.Deposit) {
                ++priorityRequestsExecuted;
            } else if (opType == Operations.OpType.PartialExit) {
                Operations.PartialExit memory op = Operations.readPartialExitPubdata(pubData);
                withdrawOrStore(op.tokenId, op.owner, op.amount);

            } else if (opType == Operations.OpType.ForcedExit) {
                Operations.ForcedExit memory op = Operations.readForcedExitPubdata(pubData);
                withdrawOrStore(op.tokenId, op.target, op.amount);

            } else if (opType == Operations.OpType.FullExit) {
                Operations.FullExit memory fullExitData = Operations.readFullExitPubdata(pubData);
                incBalanceToWithdraw(packAddressAndTokenId(fullExitData.owner, fullExitData.tokenId), fullExitData.amount);

                ++priorityRequestsExecuted;
            } else {
                revert("exe13"); // unsupported op in block execution
            }

            processableOnchainOpsHash = Utils.addBytesToHash(processableOnchainOpsHash, pubData);
        }
        require(processableOnchainOpsHash == _blockExecuteData.storedBlock.processableOnchainOperationsHash, "exe13"); // incorrect onchain ops executed
    }

    /// @notice Execute blocks, completing priority operations and processing withdrawals.
    function executeBlocks(
        ExecuteBlockInfo[] memory _blocksData
    ) external nonReentrant {
        requireActive();
        governance.requireActiveValidator(msg.sender);

        uint32 priorityRequestsExecuted = 0;
        uint32 nBlocks = uint32(_blocksData.length);
        for (uint32 i = 0; i < nBlocks; ++i) {
            priorityRequestsExecuted += executeOneBlock(_blocksData[i], i);
            emit BlockVerification(_blocksData[i].storedBlock.blockNumber);
        }

        firstPriorityRequestId += priorityRequestsExecuted;
        totalCommittedPriorityRequests -= priorityRequestsExecuted;
        totalOpenPriorityRequests -= priorityRequestsExecuted;

        totalBlocksVerified += nBlocks;
    }

    /// @notice Block verification.
    /// @notice Verify proof -> process onchain withdrawals (accrue balances from withdrawals) -> remove priority requests
    function verifyCommitments(bytes32[] calldata _commitments, uint256[] calldata) external {
        // todo recursive verifier
        hashedVerifiedCommitments[keccak256(abi.encode(_commitments))] = true;
    }


    /// @notice Reverts unverified blocks
    function revertBlocks(StoredBlockInfo[] memory _blocksToRevert) external nonReentrant {
        governance.requireActiveValidator(msg.sender);

        uint32 blocksCommitted = totalBlocksCommitted;
        uint32 blocksToRevert = Utils.minU32(uint32(_blocksToRevert.length), blocksCommitted - totalBlocksVerified);
        uint64 revertedPriorityRequests = 0;

        for (uint32 i = 0; i < blocksToRevert; ++i) {
            StoredBlockInfo memory storedBlockInfo  = _blocksToRevert[i];
            require(hashedBlocks[blocksCommitted] == hashStoredBlockInfo(storedBlockInfo), "frk10"); // incorrect stored block info

            delete hashedBlocks[blocksCommitted];

            --blocksCommitted;
            revertedPriorityRequests += storedBlockInfo.priorityOperations;
        }

        totalBlocksCommitted = blocksCommitted;
        totalCommittedPriorityRequests -= revertedPriorityRequests;

        emit BlocksRevert(totalBlocksVerified, blocksCommitted);
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

        incBalanceToWithdraw(packedBalanceKey, _amount);
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

    function emitDepositCommitEvent(uint32 _blockNumber, Operations.Deposit memory depositData) internal {
        emit DepositCommit(_blockNumber, depositData.accountId, depositData.owner, depositData.tokenId, depositData.amount);
    }

    function emitFullExitCommitEvent(uint32 _blockNumber, Operations.FullExit memory fullExitData) internal {
        emit FullExitCommit(_blockNumber, fullExitData.accountId, fullExitData.owner, fullExitData.tokenId, fullExitData.amount);
    }

    /// @notice Gets operations packed in bytes array. Unpacks it and stores onchain operations.
    /// Priority operations must be committed in the same order as they are in the priority queue.
    function collectOnchainOps(CommitBlockInfo memory _newBlockData)
        internal returns (bytes32 processableOperationsHash, uint64 priorityOperationsProcessed) {
        bytes memory pubData = _newBlockData.publicData;

        require(pubData.length % CHUNK_BYTES == 0, "fcs11"); // pubdata length must be a multiple of CHUNK_BYTES

        uint64 uncommittedPriorityRequestsOffset = firstPriorityRequestId + totalCommittedPriorityRequests;
        priorityOperationsProcessed = 0;
        processableOperationsHash = EMPTY_STRING_KECCAK;

        for (uint32 i = 0; i < _newBlockData.onchainOperations.length; ++i) {
            OnchainOperationData memory onchainOpData = _newBlockData.onchainOperations[i];
            uint pubdataOffset = onchainOpData.publicDataOffset;

            Operations.OpType opType = Operations.OpType(uint8(pubData[pubdataOffset]));

            if (opType == Operations.OpType.Deposit) {
                bytes memory opPubData = Bytes.slice(pubData, pubdataOffset, DEPOSIT_BYTES);

                Operations.Deposit memory depositData = Operations.readDepositPubdata(opPubData);
                emitDepositCommitEvent(_newBlockData.blockNumber, depositData);

                OnchainOperation memory onchainOp = OnchainOperation(
                    Operations.OpType.Deposit,
                    opPubData
                );
                commitNextPriorityOperation(onchainOp, uncommittedPriorityRequestsOffset + priorityOperationsProcessed);
                priorityOperationsProcessed++;

                processableOperationsHash = Utils.addBytesToHash(processableOperationsHash, opPubData);
            } else if (opType == Operations.OpType.PartialExit) {
                bytes memory opPubData = Bytes.slice(pubData, pubdataOffset, PARTIAL_EXIT_BYTES);

                processableOperationsHash = Utils.addBytesToHash(processableOperationsHash, opPubData);
            } else if (opType == Operations.OpType.ForcedExit) {
                bytes memory opPubData = Bytes.slice(pubData, pubdataOffset, FORCED_EXIT_BYTES);

                processableOperationsHash = Utils.addBytesToHash(processableOperationsHash, opPubData);
            } else if (opType == Operations.OpType.FullExit) {
                bytes memory opPubData = Bytes.slice(pubData, pubdataOffset, FULL_EXIT_BYTES);

                Operations.FullExit memory fullExitData = Operations.readFullExitPubdata(opPubData);
                emitFullExitCommitEvent(_newBlockData.blockNumber, fullExitData);

                OnchainOperation memory onchainOp = OnchainOperation(
                    Operations.OpType.FullExit,
                    opPubData
                );
                commitNextPriorityOperation(onchainOp, uncommittedPriorityRequestsOffset + priorityOperationsProcessed);
                priorityOperationsProcessed++;

                processableOperationsHash = Utils.addBytesToHash(processableOperationsHash, opPubData);
            } else if (opType == Operations.OpType.ChangePubKey) {
                bytes memory opPubData = Bytes.slice(pubData, pubdataOffset, CHANGE_PUBKEY_BYTES);

                Operations.ChangePubKey memory op = Operations.readChangePubKeyPubdata(opPubData);

                if (onchainOpData.ethWitness.length > 0) {
                    bool valid = verifyChangePubkeySignature(onchainOpData.ethWitness, op);
                    require(valid, "fpp15"); // failed to verify change pubkey hash signature
                } else {
                    bool valid = authFacts[op.owner][op.nonce] == keccak256(abi.encodePacked(op.pubKeyHash));
                    require(valid, "fpp16"); // new pub key hash is not authenticated properly
                }
            } else {
                revert("fpp14"); // unsupported op
            }
        }
    }

    /// @notice Checks that signature is valid for pubkey change message
    /// @param _ethWitness Signature (65 bytes) + 32bytes of the arbitrary signed data
    /// @param _changePk Parsed change pubkey operation
    function verifyChangePubkeySignature(bytes memory _ethWitness, Operations.ChangePubKey memory _changePk) internal pure returns (bool) {
        bytes memory signedMessage = abi.encodePacked("\x19Ethereum Signed Message:\n28", _changePk.pubKeyHash, _changePk.nonce, _changePk.accountId);
        (uint offset, bytes memory signature) = Bytes.read(_ethWitness, 0, 65);
        (,bytes32 additionalData) = Bytes.readBytes32(_ethWitness, offset);
        bytes32 messageHash = keccak256(abi.encodePacked(signedMessage, additionalData));
        address recoveredAddress = Utils.recoverAddressFromEthSignature(signature, messageHash);
        return recoveredAddress == _changePk.owner;
    }

    /// @dev Creates block commitment from its data
    function createBlockCommitment(
        StoredBlockInfo memory _previousBlock,
        CommitBlockInfo memory _newBlockData
    ) internal view returns (bytes32 commitment) {
        bytes32 hash = sha256(
            abi.encodePacked(uint256(_newBlockData.blockNumber), uint256(_newBlockData.feeAccount))
        );
        // TODO: add _newBlockData.onchainOperations.length
        // TODO: add _newBlockData.timestamp.length
        hash = sha256(abi.encodePacked(hash, uint256(_previousBlock.stateHash)));
        hash = sha256(abi.encodePacked(hash, uint256(_newBlockData.newStateRoot)));

        bytes memory pubdata = _newBlockData.publicData;

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
            let success := staticcall(
                gas(),
                0x2,
                pubdata,
                add(pubDataLen, 0x20),
                hashResult,
                0x20
            )
            mstore(pubdata, pubDataLen)

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

    function incBalanceToWithdraw(bytes22 _packedBalanceKey, uint128 _amount) internal {
        uint128 balance = balancesToWithdraw[_packedBalanceKey].balanceToWithdraw;
        balancesToWithdraw[_packedBalanceKey] = BalanceToWithdraw(balance.add(_amount), 0xff);
    }

    function openAndCheckCommitmentInSlot(bytes32 _commitment, bytes32[] memory _commitments, uint256 _idx) internal view returns (bool){
        return hashedVerifiedCommitments[keccak256(abi.encode(_commitments))] && _commitments[_idx] == _commitment;
    }

}
