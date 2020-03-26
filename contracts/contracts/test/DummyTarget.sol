pragma solidity 0.5.16;

interface DummyTarget {

    function upgradeNoticePeriod() external pure returns (uint);

    function get_DUMMY_INDEX() external pure returns (uint256);

    function initialize(bytes calldata initializationParameters) external;

    function verifyPriorityOperation() external;

    function readyForUpgrade() external returns (bool);

}

contract DummyFirst is DummyTarget {

    uint constant UPGRADE_NOTICE_PERIOD = 4;
    function upgradeNoticePeriod() external pure returns (uint) {
        return UPGRADE_NOTICE_PERIOD;
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

    function totalVerifiedPriorityOperations() internal returns (uint64) {
        return _verifiedPriorityOperations;
    }

    function totalRegisteredPriorityOperations() internal returns (uint64) {
        return 1;
    }

    function verifyPriorityOperation() external {
        _verifiedPriorityOperations++;
    }

    function readyForUpgrade() external returns (bool) {
        return totalVerifiedPriorityOperations() >= totalRegisteredPriorityOperations();
    }

}

contract DummySecond is DummyTarget {

    uint constant UPGRADE_NOTICE_PERIOD = 4;
    function upgradeNoticePeriod() external pure returns (uint) {
        return UPGRADE_NOTICE_PERIOD;
    }

    uint256 private constant DUMMY_INDEX = 2;
    function get_DUMMY_INDEX() external pure returns (uint256) {
        return DUMMY_INDEX;
    }

    uint64 _verifiedPriorityOperations;

    function initialize(bytes calldata initializationParameters) external {
        bytes32 byte_0 = bytes32(uint256(uint8(initializationParameters[0])));
        bytes32 byte_1 = bytes32(uint256(uint8(initializationParameters[1])));
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

    function readyForUpgrade() external returns (bool) {
        return totalVerifiedPriorityOperations() >= totalRegisteredPriorityOperations();
    }

}
