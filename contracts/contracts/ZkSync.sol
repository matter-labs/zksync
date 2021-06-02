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
import "./RegenesisMultisig.sol";
import "./AdditionalZkSync.sol";

/// @title zkSync main contract
/// @author Matter Labs
contract ZkSync is UpgradeableMaster, Storage, Config, Events, ReentrancyGuard {
    using SafeMath for uint256;
    using SafeMathUInt128 for uint128;

    bytes32 private constant EMPTY_STRING_KECCAK = 0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470;

    /// @notice Data needed to process onchain operation from block public data.
    /// @notice Onchain operations is operations that need some processing on L1: Deposits, Withdrawals, ChangePubKey.
    /// @param ethWitness Some external data that can be needed for operation processing
    /// @param publicDataOffset Byte offset in public data for onchain operation
    struct OnchainOperationData {
        bytes ethWitness;
        uint32 publicDataOffset;
    }

    /// @notice Data needed to commit new block
    struct CommitBlockInfo {
        bytes32 newStateHash;
        bytes publicData;
        uint256 timestamp;
        OnchainOperationData[] onchainOperations;
        uint32 blockNumber;
        uint32 feeAccount;
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
        return 0;
    }

    /// @notice Notification that upgrade notice period started
    /// @dev Can be external because Proxy contract intercepts illegal calls of this function
    function upgradeNoticePeriodStarted() external override {
        upgradeStartTimestamp = block.timestamp;
    }

    /// @notice Notification that upgrade preparation status is activated
    /// @dev Can be external because Proxy contract intercepts illegal calls of this function
    function upgradePreparationStarted() external override {
        upgradePreparationActive = true;
        upgradePreparationActivationTime = block.timestamp;

        require(block.timestamp >= upgradeStartTimestamp.add(approvedUpgradeNoticePeriod));
    }

    /// @notice Notification that upgrade canceled
    /// @dev Can be external because Proxy contract intercepts illegal calls of this function
    function upgradeCanceled() external override {
        upgradePreparationActive = false;
        upgradePreparationActivationTime = 0;
        approvedUpgradeNoticePeriod = UPGRADE_NOTICE_PERIOD;
        emit NoticePeriodChange(approvedUpgradeNoticePeriod);
        upgradeStartTimestamp = 0;
        for (uint256 i = 0; i < SECURITY_COUNCIL_MEMBERS_NUMBER; ++i) {
            securityCouncilApproves[i] = false;
        }
        numberOfApprovalsFromSecurityCouncil = 0;
    }

    /// @notice Notification that upgrade finishes
    /// @dev Can be external because Proxy contract intercepts illegal calls of this function
    function upgradeFinishes() external override {
        upgradePreparationActive = false;
        upgradePreparationActivationTime = 0;
        approvedUpgradeNoticePeriod = UPGRADE_NOTICE_PERIOD;
        emit NoticePeriodChange(approvedUpgradeNoticePeriod);
        upgradeStartTimestamp = 0;
        for (uint256 i = 0; i < SECURITY_COUNCIL_MEMBERS_NUMBER; ++i) {
            securityCouncilApproves[i] = false;
        }
        numberOfApprovalsFromSecurityCouncil = 0;
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

        (address _governanceAddress, address _verifierAddress, address _additionalZkSync, bytes32 _genesisStateHash) =
            abi.decode(initializationParameters, (address, address, address, bytes32));

        verifier = Verifier(_verifierAddress);
        governance = Governance(_governanceAddress);
        additionalZkSync = AdditionalZkSync(_additionalZkSync);

        StoredBlockInfo memory storedBlockZero =
            StoredBlockInfo(0, 0, EMPTY_STRING_KECCAK, 0, _genesisStateHash, bytes32(0));

        storedBlockHashes[0] = hashStoredBlockInfo(storedBlockZero);

        approvedUpgradeNoticePeriod = UPGRADE_NOTICE_PERIOD;
        emit NoticePeriodChange(approvedUpgradeNoticePeriod);
    }

    /// @notice zkSync contract upgrade. Can be external because Proxy contract intercepts illegal calls of this function.
    /// @param upgradeParameters Encoded representation of upgrade parameters
    // solhint-disable-next-line no-empty-blocks
    function upgrade(bytes calldata upgradeParameters) external nonReentrant {
        require(totalBlocksCommitted == totalBlocksProven, "wq1"); // All the blocks must be proven
        require(totalBlocksCommitted == totalBlocksExecuted, "w12"); // All the blocks must be executed

        StoredBlockInfo memory lastBlockInfo;
        (lastBlockInfo) = abi.decode(upgradeParameters, (StoredBlockInfo));
        require(storedBlockHashes[totalBlocksExecuted] == hashStoredBlockInfo(lastBlockInfo), "wqqs"); // The provided block info should be equal to the current one

        RegenesisMultisig multisig = RegenesisMultisig($$(REGENESIS_MULTISIG_ADDRESS));
        bytes32 oldRootHash = multisig.oldRootHash();
        require(oldRootHash == lastBlockInfo.stateHash, "wqqe");

        bytes32 newRootHash = multisig.newRootHash();

        // Overriding the old block's root hash with the new one
        lastBlockInfo.stateHash = newRootHash;
        storedBlockHashes[totalBlocksExecuted] = hashStoredBlockInfo(lastBlockInfo);
        additionalZkSync = AdditionalZkSync(address($$(NEW_ADDITIONAL_ZKSYNC_ADDRESS)));

        approvedUpgradeNoticePeriod = UPGRADE_NOTICE_PERIOD;
        emit NoticePeriodChange(approvedUpgradeNoticePeriod);
    }

    function cutUpgradeNoticePeriod() external {
        /// All functions delegated to additional contract should NOT be nonReentrant
        delegateAdditional();
    }

    /// @notice Sends tokens
    /// @dev NOTE: will revert if transfer call fails or rollup balance difference (before and after transfer) is bigger than _maxAmount
    /// @dev This function is used to allow tokens to spend zkSync contract balance up to amount that is requested
    /// @param _token Token address
    /// @param _to Address of recipient
    /// @param _amount Amount of tokens to transfer
    /// @param _maxAmount Maximum possible amount of tokens to transfer to this account
    function _transferERC20(
        IERC20 _token,
        address _to,
        uint128 _amount,
        uint128 _maxAmount
    ) external returns (uint128 withdrawnAmount) {
        require(msg.sender == address(this), "5"); // wtg10 - can be called only from this contract as one "external" call (to revert all this function state changes if it is needed)

        uint256 balanceBefore = _token.balanceOf(address(this));
        require(Utils.sendERC20(_token, _to, _amount), "6"); // wtg11 - ERC20 transfer fails
        uint256 balanceAfter = _token.balanceOf(address(this));
        uint256 balanceDiff = balanceBefore.sub(balanceAfter);
        require(balanceDiff <= _maxAmount, "7"); // wtg12 - rollup balance difference (before and after transfer) is bigger than _maxAmount

        return SafeCast.toUint128(balanceDiff);
    }

    /// @notice Accrues users balances from deposit priority requests in Exodus mode
    /// @dev WARNING: Only for Exodus mode
    /// @dev Canceling may take several separate transactions to be completed
    /// @param _n number of requests to process
    function cancelOutstandingDepositsForExodusMode(uint64 _n, bytes[] memory _depositsPubdata) external {
        /// All functions delegated to additional contract should NOT be nonReentrant
        delegateAdditional();
    }

    /// @notice Deposit ETH to Layer 2 - transfer ether from user into contract, validate it, register deposit
    /// @param _zkSyncAddress The receiver Layer 2 address
    function depositETH(address _zkSyncAddress) external payable {
        require(_zkSyncAddress != SPECIAL_ACCOUNT_ADDRESS, "P");
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
        require(_zkSyncAddress != SPECIAL_ACCOUNT_ADDRESS, "P");
        requireActive();

        // Get token id by its address
        uint16 tokenId = governance.validateTokenAddress(address(_token));
        require(!governance.pausedTokens(tokenId), "b"); // token deposits are paused

        uint256 balanceBefore = _token.balanceOf(address(this));
        require(Utils.transferFromERC20(_token, msg.sender, address(this), SafeCast.toUint128(_amount)), "c"); // token transfer failed deposit
        uint256 balanceAfter = _token.balanceOf(address(this));
        uint128 depositAmount = SafeCast.toUint128(balanceAfter.sub(balanceBefore));
        require(depositAmount <= MAX_DEPOSIT_AMOUNT, "C");

        registerDeposit(tokenId, depositAmount, _zkSyncAddress);
    }

    /// @notice Returns amount of tokens that can be withdrawn by `address` from zkSync contract
    /// @param _address Address of the tokens owner
    /// @param _token Address of token, zero address is used for ETH
    function getPendingBalance(address _address, address _token) public view returns (uint128) {
        uint16 tokenId = 0;
        if (_token != address(0)) {
            tokenId = governance.validateTokenAddress(_token);
        }
        return pendingBalances[packAddressAndTokenId(_address, tokenId)].balanceToWithdraw;
    }

    /// @notice  Withdraws tokens from zkSync contract to the owner
    /// @param _owner Address of the tokens owner
    /// @param _token Address of tokens, zero address is used for ETH
    /// @param _amount Amount to withdraw to request.
    ///         NOTE: We will call ERC20.transfer(.., _amount), but if according to internal logic of ERC20 token zkSync contract
    ///         balance will be decreased by value more then _amount we will try to subtract this value from user pending balance
    function withdrawPendingBalance(
        address payable _owner,
        address _token,
        uint128 _amount
    ) external nonReentrant {
        if (_token == address(0)) {
            registerWithdrawal(0, _amount, _owner);
            (bool success, ) = _owner.call{value: _amount}("");
            require(success, "d"); // ETH withdraw failed
        } else {
            uint16 tokenId = governance.validateTokenAddress(_token);
            bytes22 packedBalanceKey = packAddressAndTokenId(_owner, tokenId);
            uint128 balance = pendingBalances[packedBalanceKey].balanceToWithdraw;
            // We will allow withdrawals of `value` such that:
            // `value` <= user pending balance
            // `value` can be bigger then `_amount` requested if token takes fee from sender in addition to `_amount` requested
            uint128 withdrawnAmount = this._transferERC20(IERC20(_token), _owner, _amount, balance);
            registerWithdrawal(tokenId, withdrawnAmount, _owner);
        }
    }

    /// @notice  Withdraws NFT from zkSync contract to the owner
    /// @param _tokenId Id of NFT token
    function withdrawPendingNFTBalance(uint32 _tokenId) external nonReentrant {
        Operations.WithdrawNFT memory op = pendingWithdrawnNFTs[_tokenId];
        require(op.creatorAddress != address(0), "op"); // No NFT to withdraw
        NFTFactory _factory = governance.getNFTFactory(op.creatorAccountId, op.creatorAddress);
        _factory.mintNFTFromZkSync(
            op.creatorAddress,
            op.receiver,
            op.creatorAccountId,
            op.serialId,
            op.contentHash,
            op.tokenId
        );
        // Save withdrawn nfts for future deposits
        withdrawnNFTs[op.tokenId] = address(_factory);
        emit WithdrawalNFT(op.tokenId);
        delete pendingWithdrawnNFTs[_tokenId];
    }

    /// @notice Register full exit request - pack pubdata, add priority request
    /// @param _accountId Numerical id of the account
    /// @param _token Token address, 0 address for ether
    function requestFullExit(uint32 _accountId, address _token) public nonReentrant {
        requireActive();
        require(_accountId <= MAX_ACCOUNT_ID, "e");
        require(_accountId != SPECIAL_ACCOUNT_ID, "v"); // request full exit for nft storage account

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
                amount: 0, // unknown at this point
                nftCreatorAccountId: uint32(0), // unknown at this point
                nftCreatorAddress: address(0), // unknown at this point
                nftSerialId: uint32(0), // unknown at this point
                nftContentHash: bytes32(0) // unknown at this point
            });
        bytes memory pubData = Operations.writeFullExitPubdataForPriorityQueue(op);
        addPriorityRequest(Operations.OpType.FullExit, pubData);

        // User must fill storage slot of balancesToWithdraw(msg.sender, tokenId) with nonzero value
        // In this case operator should just overwrite this slot during confirming withdrawal
        bytes22 packedBalanceKey = packAddressAndTokenId(msg.sender, tokenId);
        pendingBalances[packedBalanceKey].gasReserveValue = FILLED_GAS_RESERVE_VALUE;
    }

    /// @notice Register full exit nft request - pack pubdata, add priority request
    /// @param _accountId Numerical id of the account
    /// @param _tokenId NFT token id in zkSync network
    function requestFullExitNFT(uint32 _accountId, uint32 _tokenId) public nonReentrant {
        requireActive();
        require(_accountId <= MAX_ACCOUNT_ID, "e");
        require(_accountId != SPECIAL_ACCOUNT_ID, "v"); // request full exit nft for nft storage account
        require(MAX_FUNGIBLE_TOKEN_ID < _tokenId && _tokenId < SPECIAL_NFT_TOKEN_ID, "T"); // request full exit nft for invalid token id

        // Priority Queue request
        Operations.FullExit memory op =
            Operations.FullExit({
                accountId: _accountId,
                owner: msg.sender,
                tokenId: _tokenId,
                amount: 0, // unknown at this point
                nftCreatorAccountId: uint32(0), // unknown at this point
                nftCreatorAddress: address(0), // unknown at this point
                nftSerialId: uint32(0), // unknown at this point
                nftContentHash: bytes32(0) // unknown at this point
            });
        bytes memory pubData = Operations.writeFullExitPubdataForPriorityQueue(op);
        addPriorityRequest(Operations.OpType.FullExit, pubData);
    }

    /// @dev Process one block commit using previous block StoredBlockInfo,
    /// @dev returns new block StoredBlockInfo
    /// @dev NOTE: Does not change storage (except events, so we can't mark it view)
    function commitOneBlock(StoredBlockInfo memory _previousBlock, CommitBlockInfo memory _newBlock)
        internal
        view
        returns (StoredBlockInfo memory storedNewBlock)
    {
        require(_newBlock.blockNumber == _previousBlock.blockNumber + 1, "f"); // only commit next block

        // Check timestamp of the new block
        {
            require(_newBlock.timestamp >= _previousBlock.timestamp, "g"); // Block should be after previous block
            bool timestampNotTooSmall = block.timestamp.sub(COMMIT_TIMESTAMP_NOT_OLDER) <= _newBlock.timestamp;
            bool timestampNotTooBig = _newBlock.timestamp <= block.timestamp.add(COMMIT_TIMESTAMP_APPROXIMATION_DELTA);
            require(timestampNotTooSmall && timestampNotTooBig, "h"); // New block timestamp is not valid
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
        require(storedBlockHashes[totalBlocksCommitted] == hashStoredBlockInfo(_lastCommittedBlockData), "i"); // incorrect previous block data

        for (uint32 i = 0; i < _newBlocksData.length; ++i) {
            _lastCommittedBlockData = commitOneBlock(_lastCommittedBlockData, _newBlocksData[i]);

            totalCommittedPriorityRequests += _lastCommittedBlockData.priorityOperations;
            storedBlockHashes[_lastCommittedBlockData.blockNumber] = hashStoredBlockInfo(_lastCommittedBlockData);

            emit BlockCommit(_lastCommittedBlockData.blockNumber);
        }

        totalBlocksCommitted += uint32(_newBlocksData.length);

        require(totalCommittedPriorityRequests <= totalOpenPriorityRequests, "j");
    }

    /// @dev 1. Try to send token to _recipients
    /// @dev 2. On failure: Increment _recipients balance to withdraw.
    function withdrawOrStoreNFT(Operations.WithdrawNFT memory op) internal {
        NFTFactory _factory = governance.getNFTFactory(op.creatorAccountId, op.creatorAddress);
        try
            _factory.mintNFTFromZkSync{gas: WITHDRAWAL_NFT_GAS_LIMIT}(
                op.creatorAddress,
                op.receiver,
                op.creatorAccountId,
                op.serialId,
                op.contentHash,
                op.tokenId
            )
        {
            // Save withdrawn nfts for future deposits
            withdrawnNFTs[op.tokenId] = address(_factory);
            emit WithdrawalNFT(op.tokenId);
        } catch {
            pendingWithdrawnNFTs[op.tokenId] = op;
        }
    }

    /// @dev 1. Try to send token to _recipients
    /// @dev 2. On failure: Increment _recipients balance to withdraw.
    function withdrawOrStore(
        uint16 _tokenId,
        address _recipient,
        uint128 _amount
    ) internal {
        bytes22 packedBalanceKey = packAddressAndTokenId(_recipient, _tokenId);

        bool sent = false;
        if (_tokenId == 0) {
            address payable toPayable = address(uint160(_recipient));
            sent = sendETHNoRevert(toPayable, _amount);
        } else {
            address tokenAddr = governance.tokenAddresses(_tokenId);
            // We use `_transferERC20` here to check that `ERC20` token indeed transferred `_amount`
            // and fail if token subtracted from zkSync balance more then `_amount` that was requested.
            // This can happen if token subtracts fee from sender while transferring `_amount` that was requested to transfer.
            try this._transferERC20{gas: WITHDRAWAL_GAS_LIMIT}(IERC20(tokenAddr), _recipient, _amount, _amount) {
                sent = true;
            } catch {
                sent = false;
            }
        }
        if (sent) {
            emit Withdrawal(_tokenId, _amount);
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
        require(_blockExecuteData.storedBlock.blockNumber == totalBlocksExecuted + _executedBlockIdx + 1, "k"); // Execute blocks in order

        bytes32 pendingOnchainOpsHash = EMPTY_STRING_KECCAK;
        for (uint32 i = 0; i < _blockExecuteData.pendingOnchainOpsPubdata.length; ++i) {
            bytes memory pubData = _blockExecuteData.pendingOnchainOpsPubdata[i];

            Operations.OpType opType = Operations.OpType(uint8(pubData[0]));

            if (opType == Operations.OpType.PartialExit) {
                Operations.PartialExit memory op = Operations.readPartialExitPubdata(pubData);
                // Circuit guarantees that partial exits are available only for fungible tokens
                require(op.tokenId <= MAX_FUNGIBLE_TOKEN_ID, "mf1");
                withdrawOrStore(uint16(op.tokenId), op.owner, op.amount);
            } else if (opType == Operations.OpType.ForcedExit) {
                Operations.ForcedExit memory op = Operations.readForcedExitPubdata(pubData);
                // Circuit guarantees that forced exits are available only for fungible tokens
                require(op.tokenId <= MAX_FUNGIBLE_TOKEN_ID, "mf2");
                withdrawOrStore(uint16(op.tokenId), op.target, op.amount);
            } else if (opType == Operations.OpType.FullExit) {
                Operations.FullExit memory op = Operations.readFullExitPubdata(pubData);
                if (op.tokenId <= MAX_FUNGIBLE_TOKEN_ID) {
                    withdrawOrStore(uint16(op.tokenId), op.owner, op.amount);
                } else {
                    if (op.amount == 1) {
                        Operations.WithdrawNFT memory withdrawNftOp =
                            Operations.WithdrawNFT(
                                op.nftCreatorAccountId,
                                op.nftCreatorAddress,
                                op.nftSerialId,
                                op.nftContentHash,
                                op.owner,
                                op.tokenId
                            );
                        withdrawOrStoreNFT(withdrawNftOp);
                    }
                }
            } else if (opType == Operations.OpType.WithdrawNFT) {
                Operations.WithdrawNFT memory op = Operations.readWithdrawNFTPubdata(pubData);
                withdrawOrStoreNFT(op);
            } else {
                revert("l"); // unsupported op in block execution
            }

            pendingOnchainOpsHash = Utils.concatHash(pendingOnchainOpsHash, pubData);
        }
        require(pendingOnchainOpsHash == _blockExecuteData.storedBlock.pendingOnchainOperationsHash, "m"); // incorrect onchain ops executed
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

        totalBlocksExecuted += nBlocks;
        require(totalBlocksExecuted <= totalBlocksProven, "n"); // Can't execute blocks more then committed and proven currently.
    }

    /// @notice Blocks commitment verification.
    /// @notice Only verifies block commitments without any other processing
    function proveBlocks(StoredBlockInfo[] memory _committedBlocks, ProofInput memory _proof) external nonReentrant {
        uint32 currentTotalBlocksProven = totalBlocksProven;
        for (uint256 i = 0; i < _committedBlocks.length; ++i) {
            require(hashStoredBlockInfo(_committedBlocks[i]) == storedBlockHashes[currentTotalBlocksProven + 1], "o1");
            ++currentTotalBlocksProven;

            require(_proof.commitments[i] & INPUT_MASK == uint256(_committedBlocks[i].commitment) & INPUT_MASK, "o"); // incorrect block commitment in proof
        }

        bool success =
            verifier.verifyAggregatedBlockProof(
                _proof.recursiveInput,
                _proof.proof,
                _proof.vkIndexes,
                _proof.commitments,
                _proof.subproofsLimbs
            );
        require(success, "p"); // Aggregated proof verification fail

        require(currentTotalBlocksProven <= totalBlocksCommitted, "q");
        totalBlocksProven = currentTotalBlocksProven;
    }

    /// @notice Reverts unverified blocks
    function revertBlocks(StoredBlockInfo[] memory _blocksToRevert) external {
        /// All functions delegated to additional contract should NOT be nonReentrant
        delegateAdditional();
    }

    /// @notice Checks if Exodus mode must be entered. If true - enters exodus mode and emits ExodusMode event.
    /// @dev Exodus mode must be entered in case of current ethereum block number is higher than the oldest
    /// @dev of existed priority requests expiration block number.
    /// @return bool flag that is true if the Exodus mode must be entered.
    function activateExodusMode() public returns (bool) {
        // #if EASY_EXODUS
        bool trigger = true;
        // #else
        bool trigger =
            block.number >= priorityRequests[firstPriorityRequestId].expirationBlock &&
                priorityRequests[firstPriorityRequestId].expirationBlock != 0;
        // #endif
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

    /// @notice Withdraws token from ZkSync to root chain in case of exodus mode. User must provide proof that he owns funds
    /// @param _storedBlockInfo Last verified block
    /// @param _owner Owner of the account
    /// @param _accountId Id of the account in the tree
    /// @param _proof Proof
    /// @param _tokenId Verified token id
    /// @param _amount Amount for owner (must be total amount, not part of it)
    function performExodus(
        StoredBlockInfo memory _storedBlockInfo,
        address _owner,
        uint32 _accountId,
        uint32 _tokenId,
        uint128 _amount,
        uint32 _nftCreatorAccountId,
        address _nftCreatorAddress,
        uint32 _nftSerialId,
        bytes32 _nftContentHash,
        uint256[] memory _proof
    ) external {
        /// All functions delegated to additional should NOT be nonReentrant
        delegateAdditional();
    }

    /// @notice Set data for changing pubkey hash using onchain authorization.
    ///         Transaction author (msg.sender) should be L2 account address
    /// @notice New pubkey hash can be reset, to do that user should send two transactions:
    ///         1) First `setAuthPubkeyHash` transaction for already used `_nonce` will set timer.
    ///         2) After `AUTH_FACT_RESET_TIMELOCK` time is passed second `setAuthPubkeyHash` transaction will reset pubkey hash for `_nonce`.
    /// @param _pubkeyHash New pubkey hash
    /// @param _nonce Nonce of the change pubkey L2 transaction
    function setAuthPubkeyHash(bytes calldata _pubkeyHash, uint32 _nonce) external {
        /// All functions delegated to additional contract should NOT be nonReentrant
        delegateAdditional();
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
        bytes memory pubData = Operations.writeDepositPubdataForPriorityQueue(op);
        addPriorityRequest(Operations.OpType.Deposit, pubData);
        emit Deposit(_tokenId, _amount);
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
        uint128 balance = pendingBalances[packedBalanceKey].balanceToWithdraw;
        pendingBalances[packedBalanceKey].balanceToWithdraw = balance.sub(_amount);
        emit Withdrawal(_token, _amount);
    }

    /// @dev Gets operations packed in bytes array. Unpacks it and stores onchain operations.
    /// @dev Priority operations must be committed in the same order as they are in the priority queue.
    /// @dev NOTE: does not change storage! (only emits events)
    /// @dev processableOperationsHash - hash of the all operations that needs to be executed  (Deposit, Exits, ChangPubKey)
    /// @dev priorityOperationsProcessed - number of priority operations processed in this block (Deposits, FullExits)
    /// @dev offsetsCommitment - array where 1 is stored in chunk where onchainOperation begins and other are 0 (used in commitments)
    function collectOnchainOps(CommitBlockInfo memory _newBlockData)
        internal
        view
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

        require(pubData.length % CHUNK_BYTES == 0, "A"); // pubdata length must be a multiple of CHUNK_BYTES
        offsetsCommitment = new bytes(pubData.length / CHUNK_BYTES);
        for (uint256 i = 0; i < _newBlockData.onchainOperations.length; ++i) {
            OnchainOperationData memory onchainOpData = _newBlockData.onchainOperations[i];

            uint256 pubdataOffset = onchainOpData.publicDataOffset;
            require(pubdataOffset < pubData.length, "A1");
            require(pubdataOffset % CHUNK_BYTES == 0, "B"); // offsets should be on chunks boundaries
            uint256 chunkId = pubdataOffset / CHUNK_BYTES;
            require(offsetsCommitment[chunkId] == 0x00, "C"); // offset commitment should be empty
            offsetsCommitment[chunkId] = bytes1(0x01);

            Operations.OpType opType = Operations.OpType(uint8(pubData[pubdataOffset]));

            if (opType == Operations.OpType.Deposit) {
                bytes memory opPubData = Bytes.slice(pubData, pubdataOffset, DEPOSIT_BYTES);

                Operations.Deposit memory depositData = Operations.readDepositPubdata(opPubData);

                checkPriorityOperation(depositData, uncommittedPriorityRequestsOffset + priorityOperationsProcessed);
                priorityOperationsProcessed++;
            } else if (opType == Operations.OpType.ChangePubKey) {
                bytes memory opPubData = Bytes.slice(pubData, pubdataOffset, CHANGE_PUBKEY_BYTES);

                Operations.ChangePubKey memory op = Operations.readChangePubKeyPubdata(opPubData);

                if (onchainOpData.ethWitness.length != 0) {
                    bool valid = verifyChangePubkey(onchainOpData.ethWitness, op);
                    require(valid, "D"); // failed to verify change pubkey hash signature
                } else {
                    bool valid = authFacts[op.owner][op.nonce] == keccak256(abi.encodePacked(op.pubKeyHash));
                    require(valid, "E"); // new pub key hash is not authenticated properly
                }
            } else {
                bytes memory opPubData;

                if (opType == Operations.OpType.PartialExit) {
                    opPubData = Bytes.slice(pubData, pubdataOffset, PARTIAL_EXIT_BYTES);
                } else if (opType == Operations.OpType.ForcedExit) {
                    opPubData = Bytes.slice(pubData, pubdataOffset, FORCED_EXIT_BYTES);
                } else if (opType == Operations.OpType.WithdrawNFT) {
                    opPubData = Bytes.slice(pubData, pubdataOffset, WITHDRAW_NFT_BYTES);
                } else if (opType == Operations.OpType.FullExit) {
                    opPubData = Bytes.slice(pubData, pubdataOffset, FULL_EXIT_BYTES);

                    Operations.FullExit memory fullExitData = Operations.readFullExitPubdata(opPubData);

                    checkPriorityOperation(
                        fullExitData,
                        uncommittedPriorityRequestsOffset + priorityOperationsProcessed
                    );
                    priorityOperationsProcessed++;
                } else {
                    revert("F"); // unsupported op
                }

                processableOperationsHash = Utils.concatHash(processableOperationsHash, opPubData);
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
        } else if (changePkType == Operations.ChangePubkeyType.OldECRECOVER) {
            return verifyChangePubkeyOldECRECOVER(_ethWitness, _changePk);
        } else {
            revert("G"); // Incorrect ChangePubKey type
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
        (, bytes memory signature) = Bytes.read(_ethWitness, 1, 65); // offset is 1 because we skip type of ChangePubkey
        //        (, bytes32 additionalData) = Bytes.readBytes32(_ethWitness, offset);
        bytes32 messageHash =
            keccak256(
                abi.encodePacked(
                    "\x19Ethereum Signed Message:\n60",
                    _changePk.pubKeyHash,
                    _changePk.nonce,
                    _changePk.accountId,
                    bytes32(0)
                )
            );
        address recoveredAddress = Utils.recoverAddressFromEthSignature(signature, messageHash);
        return recoveredAddress == _changePk.owner && recoveredAddress != address(0);
    }

    /// @notice Checks that signature is valid for pubkey change message, old version differs by form of the signed message.
    /// @param _ethWitness Signature (65 bytes)
    /// @param _changePk Parsed change pubkey operation
    function verifyChangePubkeyOldECRECOVER(bytes memory _ethWitness, Operations.ChangePubKey memory _changePk)
        internal
        pure
        returns (bool)
    {
        (, bytes memory signature) = Bytes.read(_ethWitness, 1, 65); // offset is 1 because we skip type of ChangePubkey
        bytes32 messageHash =
            keccak256(
                abi.encodePacked(
                    "\x19Ethereum Signed Message:\n152",
                    "Register zkSync pubkey:\n\n",
                    Bytes.bytesToHexASCIIBytes(abi.encodePacked(_changePk.pubKeyHash)),
                    "\n",
                    "nonce: 0x",
                    Bytes.bytesToHexASCIIBytes(Bytes.toBytesFromUInt32(_changePk.nonce)),
                    "\n",
                    "account id: 0x",
                    Bytes.bytesToHexASCIIBytes(Bytes.toBytesFromUInt32(_changePk.accountId)),
                    "\n\n",
                    "Only sign this message for a trusted client!"
                )
            );
        address recoveredAddress = Utils.recoverAddressFromEthSignature(signature, messageHash);
        return recoveredAddress == _changePk.owner && recoveredAddress != address(0);
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
        bytes32 salt = keccak256(abi.encodePacked(saltArg, _changePk.pubKeyHash));
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
        require(priorReqType == Operations.OpType.Deposit, "H"); // incorrect priority op type

        bytes20 hashedPubdata = priorityRequests[_priorityRequestId].hashedPubData;
        require(Operations.checkDepositInPriorityQueue(_deposit, hashedPubdata), "I");
    }

    /// @notice Checks that FullExit is same as operation in priority queue
    /// @param _fullExit FullExit data
    /// @param _priorityRequestId Operation's id in priority queue
    function checkPriorityOperation(Operations.FullExit memory _fullExit, uint64 _priorityRequestId) internal view {
        Operations.OpType priorReqType = priorityRequests[_priorityRequestId].opType;
        require(priorReqType == Operations.OpType.FullExit, "J"); // incorrect priority op type

        bytes20 hashedPubdata = priorityRequests[_priorityRequestId].hashedPubData;
        require(Operations.checkFullExitInPriorityQueue(_fullExit, hashedPubdata), "K");
    }

    /// @notice Checks that current state not is exodus mode
    function requireActive() internal view {
        require(!exodusMode, "L"); // exodus mode activated
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

    function increaseBalanceToWithdraw(bytes22 _packedBalanceKey, uint128 _amount) internal {
        uint128 balance = pendingBalances[_packedBalanceKey].balanceToWithdraw;
        pendingBalances[_packedBalanceKey] = PendingBalance(balance.add(_amount), FILLED_GAS_RESERVE_VALUE);
    }

    /// @notice Sends ETH
    /// @param _to Address of recipient
    /// @param _amount Amount of tokens to transfer
    /// @return bool flag indicating that transfer is successful
    function sendETHNoRevert(address payable _to, uint256 _amount) internal returns (bool) {
        (bool callSuccess, ) = _to.call{gas: WITHDRAWAL_GAS_LIMIT, value: _amount}("");
        return callSuccess;
    }

    /// @notice Delegates the call to the additional part of the main contract.
    /// @notice Should be only use to delegate the external calls as it passes the calldata
    /// @notice All functions delegated to additional contract should NOT be nonReentrant
    function delegateAdditional() internal {
        address _target = address(additionalZkSync);
        assembly {
            // The pointer to the free memory slot
            let ptr := mload(0x40)
            // Copy function signature and arguments from calldata at zero position into memory at pointer position
            calldatacopy(ptr, 0x0, calldatasize())
            // Delegatecall method of the implementation contract, returns 0 on error
            let result := delegatecall(gas(), _target, ptr, calldatasize(), 0x0, 0)
            // Get the size of the last return data
            let size := returndatasize()
            // Copy the size length of bytes from return data at zero position to pointer position
            returndatacopy(ptr, 0x0, size)

            // Depending on result value
            switch result
                case 0 {
                    // End execution and revert state changes
                    revert(ptr, size)
                }
                default {
                    // Return data with length of size at pointers position
                    return(ptr, size)
                }
        }
    }
}
