// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "./Ownable.sol";

/// @title Regenesis Multisig contract
/// @author Matter Labs
contract RegenesisMultisig is Ownable, Config {
    bytes32 public oldRootHash;
    bytes32 public newRootHash;

    bytes32 public candidateOldRootHash;
    bytes32 public candidateNewRootHash;

    /// @dev Stores boolean flags which means the confirmations of the upgrade for each member of security council
    mapping(uint256 => bool) internal securityCouncilApproves;
    uint256 internal numberOfApprovalsFromSecurityCouncil;

    // @dev The number of security council confirmatiosn needed is the same as for setting the upgrade for 1 week
    uint256 internal constant SECURITY_COUNCIL_THRESHOLD = $$(SECURITY_COUNCIL_1_WEEK_THRESHOLD);

    constructor(uint256 _numberOfApprovals) Ownable(msg.sender) {
        numberOfApprovalsFromSecurityCouncil = _numberOfApprovals;
    }

    function submitHash(bytes32 _oldRootHash, bytes32 _newRootHash) external {
        // Only zkSync team can submit the hashes
        require(msg.sender == getMaster(), "1");

        candidateOldRootHash = _oldRootHash;
        candidateNewRootHash = _newRootHash;

        oldRootHash = bytes32(0);
        newRootHash = bytes32(0);

        for (uint256 i = 0; i < SECURITY_COUNCIL_MEMBERS_NUMBER; ++i) {
            securityCouncilApproves[i] = false;
        }
        numberOfApprovalsFromSecurityCouncil = 0;
    }

    function approveHash() external {
        address payable[SECURITY_COUNCIL_MEMBERS_NUMBER] memory SECURITY_COUNCIL_MEMBERS =
            [$(SECURITY_COUNCIL_MEMBERS)];
        for (uint256 id = 0; id < SECURITY_COUNCIL_MEMBERS_NUMBER; ++id) {
            if (SECURITY_COUNCIL_MEMBERS[id] == msg.sender) {
                require(securityCouncilApproves[id] == false);
                securityCouncilApproves[id] = true;
                numberOfApprovalsFromSecurityCouncil++;

                if (numberOfApprovalsFromSecurityCouncil >= SECURITY_COUNCIL_THRESHOLD) {
                    oldRootHash = candidateOldRootHash;
                    newRootHash = candidateNewRootHash;
                }
            }
        }
    }
}
