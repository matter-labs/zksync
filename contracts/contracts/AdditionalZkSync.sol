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

/// @title zkSync additional main contract
/// @author Matter Labs
contract AdditionalZkSync is Storage, Config, Events, ReentrancyGuard {
    using SafeMath for uint256;
    using SafeMathUInt128 for uint128;

    function increaseBalanceToWithdraw(bytes22 _packedBalanceKey, uint128 _amount) internal {
        uint128 balance = pendingBalances[_packedBalanceKey].balanceToWithdraw;
        pendingBalances[_packedBalanceKey] = PendingBalance(balance.add(_amount), FILLED_GAS_RESERVE_VALUE);
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
        require(_accountId <= MAX_ACCOUNT_ID, "e");
        require(_accountId != SPECIAL_ACCOUNT_ID, "v");
        require(_tokenId < SPECIAL_NFT_TOKEN_ID, "T");

        require(exodusMode, "s"); // must be in exodus mode
        require(!performedExodus[_accountId][_tokenId], "t"); // already exited
        require(storedBlockHashes[totalBlocksExecuted] == hashStoredBlockInfo(_storedBlockInfo), "u"); // incorrect stored block info

        bool proofCorrect = verifier.verifyExitProof(
            _storedBlockInfo.stateHash,
            _accountId,
            _owner,
            _tokenId,
            _amount,
            _nftCreatorAccountId,
            _nftCreatorAddress,
            _nftSerialId,
            _nftContentHash,
            _proof
        );
        require(proofCorrect, "x");

        if (_tokenId <= MAX_FUNGIBLE_TOKEN_ID) {
            bytes22 packedBalanceKey = packAddressAndTokenId(_owner, uint16(_tokenId));
            increaseBalanceToWithdraw(packedBalanceKey, _amount);
            emit WithdrawalPending(uint16(_tokenId), _amount);
        } else {
            require(_amount != 0, "Z"); // Unsupported nft amount
            Operations.WithdrawNFT memory withdrawNftOp = Operations.WithdrawNFT(
                _nftCreatorAccountId,
                _nftCreatorAddress,
                _nftSerialId,
                _nftContentHash,
                _owner,
                _tokenId
            );
            pendingWithdrawnNFTs[_tokenId] = withdrawNftOp;
            emit WithdrawalNFTPending(_tokenId);
        }
        performedExodus[_accountId][_tokenId] = true;
    }

    function cancelOutstandingDepositsForExodusMode(uint64 _n, bytes[] memory _depositsPubdata) external {
        require(exodusMode, "8"); // exodus mode not active
        uint64 toProcess = Utils.minU64(totalOpenPriorityRequests, _n);
        require(toProcess > 0, "9"); // no deposits to process
        uint64 currentDepositIdx = 0;
        for (uint64 id = firstPriorityRequestId; id < firstPriorityRequestId + toProcess; id++) {
            if (priorityRequests[id].opType == Operations.OpType.Deposit) {
                bytes memory depositPubdata = _depositsPubdata[currentDepositIdx];
                require(Utils.hashBytesToBytes20(depositPubdata) == priorityRequests[id].hashedPubData, "a");
                ++currentDepositIdx;

                Operations.Deposit memory op = Operations.readDepositPubdata(depositPubdata);
                bytes22 packedBalanceKey = packAddressAndTokenId(op.owner, uint16(op.tokenId));
                pendingBalances[packedBalanceKey].balanceToWithdraw += op.amount;
            }
            delete priorityRequests[id];
        }
        firstPriorityRequestId += toProcess;
        totalOpenPriorityRequests -= toProcess;
    }

    uint256 internal constant SECURITY_COUNCIL_THRESHOLD = $$(SECURITY_COUNCIL_THRESHOLD);

    function approvedCutUpgradeNoticePeriod(address addr) internal {
        address payable[SECURITY_COUNCIL_MEMBERS_NUMBER] memory SECURITY_COUNCIL_MEMBERS = [
            $(SECURITY_COUNCIL_MEMBERS)
        ];
        for (uint256 id = 0; id < SECURITY_COUNCIL_MEMBERS_NUMBER; ++id) {
            if (SECURITY_COUNCIL_MEMBERS[id] == addr && !securityCouncilApproves[id]) {
                securityCouncilApproves[id] = true;
                numberOfApprovalsFromSecurityCouncil++;

                if (numberOfApprovalsFromSecurityCouncil == SECURITY_COUNCIL_THRESHOLD) {
                    if (approvedUpgradeNoticePeriod > 0) {
                        approvedUpgradeNoticePeriod = 0;
                        emit NoticePeriodChange(approvedUpgradeNoticePeriod);
                    }
                }

                break;
            }
        }
    }

    function cutUpgradeNoticePeriod() external {
        requireActive();
        require(upgradeStartTimestamp != 0);

        approvedCutUpgradeNoticePeriod(msg.sender);
    }

    function cutUpgradeNoticePeriodBySignature(bytes[] calldata signatures) external {
        requireActive();
        require(upgradeStartTimestamp != 0);

        address gatekeeper = 0x38A43F4330f24fe920F943409709fc9A6084C939;
        (, bytes memory newTarget0) = gatekeeper.call(abi.encodeWithSignature("nextTargets(uint256)", 0));
        (, bytes memory newTarget1) = gatekeeper.call(abi.encodeWithSignature("nextTargets(uint256)", 1));
        (, bytes memory newTarget2) = gatekeeper.call(abi.encodeWithSignature("nextTargets(uint256)", 2));

        bytes32 targetsHash = keccak256(abi.encodePacked(newTarget0, newTarget1, newTarget2));
        bytes32 messageHash = keccak256(
            abi.encodePacked(
                "\x19Ethereum Signed Message:\n110",
                "Approved new ZkSync's target contracts hash\n0x",
                Bytes.bytesToHexASCIIBytes(abi.encodePacked(targetsHash))
            )
        );

        for (uint256 i = 0; i < signatures.length; ++i) {
            address recoveredAddress = Utils.recoverAddressFromEthSignature(signatures[i], messageHash);
            approvedCutUpgradeNoticePeriod(recoveredAddress);
        }
    }

    /// @notice Set data for changing pubkey hash using onchain authorization.
    ///         Transaction author (msg.sender) should be L2 account address
    /// @notice New pubkey hash can be reset, to do that user should send two transactions:
    ///         1) First `setAuthPubkeyHash` transaction for already used `_nonce` will set timer.
    ///         2) After `AUTH_FACT_RESET_TIMELOCK` time is passed second `setAuthPubkeyHash` transaction will reset pubkey hash for `_nonce`.
    /// @param _pubkeyHash New pubkey hash
    /// @param _nonce Nonce of the change pubkey L2 transaction
    function setAuthPubkeyHash(bytes calldata _pubkeyHash, uint32 _nonce) external {
        requireActive();

        require(_pubkeyHash.length == PUBKEY_HASH_BYTES, "y"); // PubKeyHash should be 20 bytes.
        if (authFacts[msg.sender][_nonce] == bytes32(0)) {
            authFacts[msg.sender][_nonce] = keccak256(_pubkeyHash);
        } else {
            uint256 currentResetTimer = authFactsResetTimer[msg.sender][_nonce];
            if (currentResetTimer == 0) {
                authFactsResetTimer[msg.sender][_nonce] = block.timestamp;
            } else {
                require(block.timestamp.sub(currentResetTimer) >= AUTH_FACT_RESET_TIMELOCK, "z");
                authFactsResetTimer[msg.sender][_nonce] = 0;
                authFacts[msg.sender][_nonce] = keccak256(_pubkeyHash);
            }
        }
    }

    /// @notice Reverts unverified blocks
    function revertBlocks(StoredBlockInfo[] memory _blocksToRevert) external {
        requireActive();

        governance.requireActiveValidator(msg.sender);

        uint32 blocksCommitted = totalBlocksCommitted;
        uint32 blocksToRevert = Utils.minU32(uint32(_blocksToRevert.length), blocksCommitted - totalBlocksExecuted);
        uint64 revertedPriorityRequests = 0;

        for (uint32 i = 0; i < blocksToRevert; ++i) {
            StoredBlockInfo memory storedBlockInfo = _blocksToRevert[i];
            require(storedBlockHashes[blocksCommitted] == hashStoredBlockInfo(storedBlockInfo), "r"); // incorrect stored block info

            delete storedBlockHashes[blocksCommitted];

            --blocksCommitted;
            revertedPriorityRequests += storedBlockInfo.priorityOperations;
        }

        totalBlocksCommitted = blocksCommitted;
        totalCommittedPriorityRequests -= revertedPriorityRequests;
        if (totalBlocksCommitted < totalBlocksProven) {
            totalBlocksProven = totalBlocksCommitted;
        }

        emit BlocksRevert(totalBlocksExecuted, blocksCommitted);
    }
}
