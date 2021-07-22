// SPDX-License-Identifier: UNLICENSED

pragma solidity ^0.7.0;

import "../Upgradeable.sol";
import "../UpgradeableMaster.sol";

interface DummyTarget {
    function getDummyIndex() external pure returns (uint256);

    function initialize(bytes calldata initializationParameters) external;

    function upgrade(bytes calldata upgradeParameters) external;

    function verifyPriorityOperation() external;
}

contract DummyFirst is UpgradeableMaster, DummyTarget {
    uint256 private constant UPGRADE_NOTICE_PERIOD = 4;

    function getNoticePeriod() external pure override returns (uint256) {
        return UPGRADE_NOTICE_PERIOD;
    }

    // solhint-disable-next-line no-empty-blocks
    function upgradeNoticePeriodStarted() external override {}

    // solhint-disable-next-line no-empty-blocks
    function upgradePreparationStarted() external override {}

    // solhint-disable-next-line no-empty-blocks
    function upgradeCanceled() external override {}

    // solhint-disable-next-line no-empty-blocks
    function upgradeFinishes() external override {}

    function isReadyForUpgrade() external view override returns (bool) {
        return totalVerifiedPriorityOperations() >= totalRegisteredPriorityOperations();
    }

    uint256 private constant DUMMY_INDEX = 1;

    function getDummyIndex() external pure override returns (uint256) {
        return DUMMY_INDEX;
    }

    uint64 private _verifiedPriorityOperations;

    function initialize(bytes calldata initializationParameters) external override {
        bytes32 byteZero = bytes32(uint256(uint8(initializationParameters[0])));
        bytes32 byteOne = bytes32(uint256(uint8(initializationParameters[1])));
        assembly {
            sstore(1, byteZero)
            sstore(2, byteOne)
        }
    }

    // solhint-disable-next-line no-empty-blocks
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
    uint256 private constant UPGRADE_NOTICE_PERIOD = 4;

    function getNoticePeriod() external pure override returns (uint256) {
        return UPGRADE_NOTICE_PERIOD;
    }

    // solhint-disable-next-line no-empty-blocks
    function upgradeNoticePeriodStarted() external override {}

    // solhint-disable-next-line no-empty-blocks
    function upgradePreparationStarted() external override {}

    // solhint-disable-next-line no-empty-blocks
    function upgradeCanceled() external override {}

    // solhint-disable-next-line no-empty-blocks
    function upgradeFinishes() external override {}

    function isReadyForUpgrade() external view override returns (bool) {
        return totalVerifiedPriorityOperations() >= totalRegisteredPriorityOperations();
    }

    uint256 private constant DUMMY_INDEX = 2;

    function getDummyIndex() external pure override returns (uint256) {
        return DUMMY_INDEX;
    }

    uint64 private _verifiedPriorityOperations;

    function initialize(bytes calldata) external pure override {
        revert("dsini");
    }

    function upgrade(bytes calldata upgradeParameters) external override {
        bytes32 byteZero = bytes32(uint256(uint8(upgradeParameters[0])));
        bytes32 byteOne = bytes32(uint256(uint8(upgradeParameters[1])));
        assembly {
            sstore(2, byteZero)
            sstore(3, byteOne)
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
