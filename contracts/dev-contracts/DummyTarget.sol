pragma solidity ^0.5.0;

import "../contracts/Upgradeable.sol";
import "../contracts/UpgradeableMaster.sol";


interface DummyTarget {

    function get_DUMMY_INDEX() external pure returns (uint256);

    function initialize(bytes calldata initializationParameters) external;

    function upgrade(bytes calldata upgradeParameters) external;

    function verifyPriorityOperation() external;

}

contract DummyFirst is UpgradeableMaster, DummyTarget {

    uint constant UPGRADE_NOTICE_PERIOD = 4;
    function get_UPGRADE_NOTICE_PERIOD() external pure returns (uint) {
        return UPGRADE_NOTICE_PERIOD;
    }

    function getNoticePeriod() external returns (uint) {
        return UPGRADE_NOTICE_PERIOD;
    }

    function upgradeNoticePeriodStarted() external {}

    function upgradePreparationStarted() external {}

    function upgradeCanceled() external {}

    function upgradeFinishes() external {}

    function isReadyForUpgrade() external returns (bool) {
        return totalVerifiedPriorityOperations() >= totalRegisteredPriorityOperations();
    }

    uint256 private constant DUMMY_INDEX = 1;
    function get_DUMMY_INDEX() external pure returns (uint256) {
        return DUMMY_INDEX;
    }

    uint64 _verifiedPriorityOperations;

    function initialize(bytes calldata initializationParameters) external {
        bytes32 byte_0 = bytes32(uint256(uint8(initializationParameters[0])));
        bytes32 byte_1 = bytes32(uint256(uint8(initializationParameters[1])));
        assembly {
            sstore(1, byte_0)
            sstore(2, byte_1)
        }
    }

    function upgrade(bytes calldata upgradeParameters) external {

    }

    function totalVerifiedPriorityOperations() internal returns (uint64) {
        return _verifiedPriorityOperations;
    }

    function totalRegisteredPriorityOperations() internal returns (uint64) {
        return 1;
    }

    function verifyPriorityOperation() external {
        _verifiedPriorityOperations++;
    }

}

contract DummySecond is UpgradeableMaster, DummyTarget {

    uint constant UPGRADE_NOTICE_PERIOD = 4;
    function get_UPGRADE_NOTICE_PERIOD() external pure returns (uint) {
        return UPGRADE_NOTICE_PERIOD;
    }

    function getNoticePeriod() external returns (uint) {
        return UPGRADE_NOTICE_PERIOD;
    }

    function upgradeNoticePeriodStarted() external {}

    function upgradePreparationStarted() external {}

    function upgradeCanceled() external {}

    function upgradeFinishes() external {}

    function isReadyForUpgrade() external returns (bool) {
        return totalVerifiedPriorityOperations() >= totalRegisteredPriorityOperations();
    }

    uint256 private constant DUMMY_INDEX = 2;
    function get_DUMMY_INDEX() external pure returns (uint256) {
        return DUMMY_INDEX;
    }

    uint64 _verifiedPriorityOperations;

    function initialize(bytes calldata initializationParameters) external {
        revert("dsini");
    }

    function upgrade(bytes calldata upgradeParameters) external {
        bytes32 byte_0 = bytes32(uint256(uint8(upgradeParameters[0])));
        bytes32 byte_1 = bytes32(uint256(uint8(upgradeParameters[1])));
        assembly {
            sstore(2, byte_0)
            sstore(3, byte_1)
        }
    }

    function totalVerifiedPriorityOperations() internal returns (uint64) {
        return _verifiedPriorityOperations;
    }

    function totalRegisteredPriorityOperations() internal returns (uint64) {
        return 0;
    }

    function verifyPriorityOperation() external {
        _verifiedPriorityOperations++;
    }

}
