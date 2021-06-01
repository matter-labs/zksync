// SPDX-License-Identifier: MIT OR Apache-2.0

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "./Ownable.sol";
import "./Config.sol";

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

    uint256 securityCouncilThreshold;

    constructor(uint256 threshold) Ownable(msg.sender) {
        securityCouncilThreshold = threshold;
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

    function approveHash(bytes32 oldRootHash, bytes32, newRootHash) external {
        require(newRootHash == candidateNewRootHash, "2");
        require(oldRootHash == candidateOldRootHash, "3");

        address payable[SECURITY_COUNCIL_MEMBERS_NUMBER] memory SECURITY_COUNCIL_MEMBERS =
            [$(SECURITY_COUNCIL_MEMBERS)];
        for (uint256 id = 0; id < SECURITY_COUNCIL_MEMBERS_NUMBER; ++id) {
            if (SECURITY_COUNCIL_MEMBERS[id] == msg.sender) {
                require(securityCouncilApproves[id] == false);
                securityCouncilApproves[id] = true;
                numberOfApprovalsFromSecurityCouncil++;

                if (numberOfApprovalsFromSecurityCouncil >= securityCouncilThreshold) {
                    oldRootHash = candidateOldRootHash;
                    newRootHash = candidateNewRootHash;
                }
            }
        }
    }
}
