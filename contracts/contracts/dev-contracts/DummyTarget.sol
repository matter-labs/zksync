// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

import "../Upgradeable.sol";
import "../UpgradeableMaster.sol";

interface DummyTarget {
    function get_DUMMY_INDEX() external pure returns (uint256);

    function initialize(bytes calldata initializationParameters) external;

    function upgrade(bytes calldata upgradeParameters) external;

    function verifyPriorityOperation() external;
}

contract DummyFirst is UpgradeableMaster, DummyTarget {
    uint256 constant UPGRADE_NOTICE_PERIOD = 4;

    function get_UPGRADE_NOTICE_PERIOD() external pure returns (uint256) {
        return UPGRADE_NOTICE_PERIOD;
    }

    function getNoticePeriod() external pure override returns (uint256) {
        return UPGRADE_NOTICE_PERIOD;
    }

    function upgradeNoticePeriodStarted() external override {}

    function upgradePreparationStarted() external override {}

    function upgradeCanceled() external override {}

    function upgradeFinishes() external override {}

    function isReadyForUpgrade() external view override returns (bool) {
        return totalVerifiedPriorityOperations() >= totalRegisteredPriorityOperations();
    }

    uint256 private constant DUMMY_INDEX = 1;

    function get_DUMMY_INDEX() external pure override returns (uint256) {
        return DUMMY_INDEX;
    }

    uint64 _verifiedPriorityOperations;

    function initialize(bytes calldata initializationParameters) external override {
        bytes32 byte_0 = bytes32(uint256(uint8(initializationParameters[0])));
        bytes32 byte_1 = bytes32(uint256(uint8(initializationParameters[1])));
        assembly {
            sstore(1, byte_0)
            sstore(2, byte_1)
        }
    }

    function upgrade(bytes calldata upgradeParameters) external override {}

    function totalVerifiedPriorityOperations() internal view returns (uint64) {
        return _verifiedPriorityOperations;
    }

    function totalRegisteredPriorityOperations() internal pure returns (uint64) {
        return 1;
    }

    function verifyPriorityOperation() external override {
        _verifiedPriorityOperations++;
    }
}

contract DummySecond is UpgradeableMaster, DummyTarget {
    uint256 constant UPGRADE_NOTICE_PERIOD = 4;

    function get_UPGRADE_NOTICE_PERIOD() external pure returns (uint256) {
        return UPGRADE_NOTICE_PERIOD;
    }

    function getNoticePeriod() external pure override returns (uint256) {
        return UPGRADE_NOTICE_PERIOD;
    }

    function upgradeNoticePeriodStarted() external override {}

    function upgradePreparationStarted() external override {}

    function upgradeCanceled() external override {}

    function upgradeFinishes() external override {}

    function isReadyForUpgrade() external view override returns (bool) {
        return totalVerifiedPriorityOperations() >= totalRegisteredPriorityOperations();
    }

    uint256 private constant DUMMY_INDEX = 2;

    function get_DUMMY_INDEX() external pure override returns (uint256) {
        return DUMMY_INDEX;
    }

    uint64 _verifiedPriorityOperations;

    function initialize(bytes calldata) external pure override {
        revert("dsini");
    }

    function upgrade(bytes calldata upgradeParameters) external override {
        bytes32 byte_0 = bytes32(uint256(uint8(upgradeParameters[0])));
        bytes32 byte_1 = bytes32(uint256(uint8(upgradeParameters[1])));
        assembly {
            sstore(2, byte_0)
            sstore(3, byte_1)
        }
    }

    function totalVerifiedPriorityOperations() internal view returns (uint64) {
        return _verifiedPriorityOperations;
    }

    function totalRegisteredPriorityOperations() internal pure returns (uint64) {
        return 0;
    }

    function verifyPriorityOperation() external override {
        _verifiedPriorityOperations++;
    }
}
