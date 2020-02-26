pragma solidity 0.5.16;

interface DummyTarget {

    function get_DUMMY_INDEX() external view returns (uint256);

    function initialize(address _address, bytes calldata _initializationParameters) external;

    function readyToBeUpgraded() external returns (bool);

}

contract DummyFirst is DummyTarget {

    uint256 private constant DUMMY_INDEX = 1;
    function get_DUMMY_INDEX() external view returns (uint256) {
        return DUMMY_INDEX;
    }

    function initialize(address _address, bytes calldata _initializationParameters) external {
        bytes memory _initializationParameters = _initializationParameters;
        bytes32 byte_0 = bytes32(uint256(uint8(_initializationParameters[0])));
        bytes32 byte_1 = bytes32(uint256(uint8(_initializationParameters[1])));
        assembly {
            sstore(0, _address)
            sstore(1, byte_0)
            sstore(2, byte_1)
        }
    }

    function readyToBeUpgraded() external returns (bool) {
        return true;
    }

}

contract DummySecond is DummyTarget {

    uint256 private constant DUMMY_INDEX = 2;
    function get_DUMMY_INDEX() external view returns (uint256) {
        return DUMMY_INDEX;
    }

    function initialize(address _address, bytes calldata _initializationParameters) external {
        bytes memory _initializationParameters = _initializationParameters;
        bytes32 byte_0 = bytes32(uint256(uint8(_initializationParameters[0])));
        bytes32 byte_1 = bytes32(uint256(uint8(_initializationParameters[1])));
        assembly {
            sstore(0, _address)
            sstore(2, byte_0)
            sstore(3, byte_1)
        }
    }

    function readyToBeUpgraded() external returns (bool) {
        return false;
    }

}
