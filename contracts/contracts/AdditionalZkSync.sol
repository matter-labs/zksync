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

interface IL1ClaimDistributor {
    function MERKLE_ROOT() external view returns (bytes32);
}

/// @title zkSync additional main contract
/// @author Matter Labs
contract AdditionalZkSync is Storage, Config, Events, ReentrancyGuard {
    using SafeMath for uint256;
    using SafeMathUInt128 for uint128;


    function increaseBalanceToWithdraw(bytes22 _packedBalanceKey, uint128 _amount) internal {
        uint128 balance = pendingBalances[_packedBalanceKey].balanceToWithdraw;
        pendingBalances[_packedBalanceKey] = PendingBalance(balance.add(_amount), FILLED_GAS_RESERVE_VALUE);
    }

    function migrateToken(address token) external nonReentrant {
        require(l1ClaimDistributor != address(0), "tm1");
        require(!migratedTokensByAddress[token], "tm3");

        _processMigration(token);
        migratedTokensByAddress[token] = true;

        emit TokenMigrationExecuted(token);
    }

    function _processMigration(address token) internal {
        if (token == address(0)) {
            uint256 total = address(this).balance;
            require(total > 0, "tm5");
            (bool success, ) = payable(l1ClaimDistributor).call{value: total}("");
            require(success, "tm9");
            return;
        }

        governance.validateTokenAddress(token);
        IERC20 erc20 = IERC20(token);
        uint256 balanceBefore = erc20.balanceOf(address(this));
        require(balanceBefore > 0, "tm5");
        erc20.transfer(l1ClaimDistributor, balanceBefore);
        uint256 balanceAfter = erc20.balanceOf(address(this));
        require(balanceBefore.sub(balanceAfter) == balanceBefore, "tm9");
    }

    uint256 internal constant SECURITY_COUNCIL_THRESHOLD = $$(SECURITY_COUNCIL_THRESHOLD);

    /// @notice processing new approval of decrease upgrade notice period time to zero
    /// @param addr address of the account that approved the reduction of the upgrade notice period to zero
    /// NOTE: does NOT revert if the address is not a security council member or number of approvals is already sufficient
    function approveCutUpgradeNoticePeriod(address addr) internal {
        address payable[SECURITY_COUNCIL_MEMBERS_NUMBER] memory SECURITY_COUNCIL_MEMBERS = [
            $(SECURITY_COUNCIL_MEMBERS)
        ];
        for (uint256 id = 0; id < SECURITY_COUNCIL_MEMBERS_NUMBER; ++id) {
            if (SECURITY_COUNCIL_MEMBERS[id] == addr) {
                // approve cut upgrade notice period if needed
                if (!securityCouncilApproves[id]) {
                    securityCouncilApproves[id] = true;
                    numberOfApprovalsFromSecurityCouncil += 1;
                    emit ApproveCutUpgradeNoticePeriod(addr);

                    if (numberOfApprovalsFromSecurityCouncil >= SECURITY_COUNCIL_THRESHOLD) {
                        if (approvedUpgradeNoticePeriod > 0) {
                            approvedUpgradeNoticePeriod = 0;
                            emit NoticePeriodChange(approvedUpgradeNoticePeriod);
                        }
                    }
                }

                break;
            }
        }
    }

    /// @notice approve to decrease upgrade notice period time to zero
    /// NOTE: сan only be called after the start of the upgrade
    function cutUpgradeNoticePeriod(bytes32 targetsHash) external nonReentrant {
        require(upgradeStartTimestamp != 0, "p1");
        require(getUpgradeTargetsHash() == targetsHash, "p3"); // given targets are not in the active upgrade

        approveCutUpgradeNoticePeriod(msg.sender);
    }

    /// @notice approve to decrease upgrade notice period time to zero by signatures
    /// NOTE: Can accept many signatures at a time, thus it is possible
    /// to completely cut the upgrade notice period in one transaction
    function cutUpgradeNoticePeriodBySignature(bytes[] calldata signatures) external nonReentrant {
        require(upgradeStartTimestamp != 0, "p2");

        bytes32 targetsHash = getUpgradeTargetsHash();
        // The Message includes a hash of the addresses of the contracts to which the upgrade will take place to prevent reuse signature.
        bytes32 messageHash = keccak256(
            abi.encodePacked(
                "\x19Ethereum Signed Message:\n110",
                "Approved new ZkSync's target contracts hash\n0x",
                Bytes.bytesToHexASCIIBytes(abi.encodePacked(targetsHash))
            )
        );

        for (uint256 i = 0; i < signatures.length; ++i) {
            address recoveredAddress = Utils.recoverAddressFromEthSignature(signatures[i], messageHash);
            approveCutUpgradeNoticePeriod(recoveredAddress);
        }
    }

    /// @return hash of the concatenation of targets for which there is an upgrade
    /// NOTE: revert if upgrade is not active at this moment
    function getUpgradeTargetsHash() internal view returns (bytes32) {
        // Get the addresses of contracts that are being prepared for the upgrade.
        address gatekeeper = $(UPGRADE_GATEKEEPER_ADDRESS);
        (bool success0, bytes memory newTarget0) = gatekeeper.staticcall(
            abi.encodeWithSignature("nextTargets(uint256)", 0)
        );
        (bool success1, bytes memory newTarget1) = gatekeeper.staticcall(
            abi.encodeWithSignature("nextTargets(uint256)", 1)
        );
        (bool success2, bytes memory newTarget2) = gatekeeper.staticcall(
            abi.encodeWithSignature("nextTargets(uint256)", 2)
        );

        require(success0 && success1 && success2, "p5"); // failed to get new targets
        address newTargetAddress0 = abi.decode(newTarget0, (address));
        address newTargetAddress1 = abi.decode(newTarget1, (address));
        address newTargetAddress2 = abi.decode(newTarget2, (address));

        return keccak256(abi.encodePacked(newTargetAddress0, newTargetAddress1, newTargetAddress2));
    }

    /// @notice Set data for changing pubkey hash using onchain authorization.
    ///         Transaction author (msg.sender) should be L2 account address
    /// @notice New pubkey hash can be reset, to do that user should send two transactions:
    ///         1) First `setAuthPubkeyHash` transaction for already used `_nonce` will set timer.
    ///         2) After `AUTH_FACT_RESET_TIMELOCK` time is passed second `setAuthPubkeyHash` transaction will reset pubkey hash for `_nonce`.
    /// @param _pubkeyHash New pubkey hash
    /// @param _nonce Nonce of the change pubkey L2 transaction
    function setAuthPubkeyHash(bytes calldata _pubkeyHash, uint32 _nonce) external nonReentrant {
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
    function revertBlocks(StoredBlockInfo[] calldata _blocksToRevert) external nonReentrant {
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
