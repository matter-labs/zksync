// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

pragma experimental ABIEncoderV2;

import "../AdditionalZkSync.sol";

contract AdditionalZkSyncCutNoticePeriodUnitTest is AdditionalZkSync {
    constructor() {
        initializeReentrancyGuard();
    }

    function enableUpgradeFromScratch() external {
        upgradePreparationActive = false;
        upgradePreparationActivationTime = 0;
        approvedUpgradeNoticePeriod = 14 days;
        upgradeStartTimestamp = block.timestamp;
        for (uint256 i = 0; i < SECURITY_COUNCIL_MEMBERS_NUMBER; ++i) {
            securityCouncilApproves[i] = false;
        }
        numberOfApprovalsFromSecurityCouncil = 0;
    }

    function disableUpgrade() external {
        upgradeStartTimestamp = 0;
    }

    function getApprovedUpgradeNoticePeriod() external view returns (uint256) {
        return approvedUpgradeNoticePeriod;
    }
}
