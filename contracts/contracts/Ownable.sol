pragma solidity 0.5.16;

/// @title Ownable Contract
/// @author Matter Labs
contract Ownable {
    /// @notice Storage position of the master address
    bytes32 private constant masterPosition = keccak256("master");

    /// @notice Contract constructor
    /// @dev Sets msg sender address as master address
    constructor() public {
        setMaster(msg.sender);
    }

    /// @notice Check if specified address is master
    /// @param _address Address to check
    function requireMaster(address _address) internal view {
        require(
            _address == getMaster(),
            "oro11"
        ); // oro11 - only by master
    }

    /// @notice Returns contract master address
    /// @return Master address
    function getMaster() public view returns (address master) {
        bytes32 position = masterPosition;
        assembly {
            master := sload(position)
        }
    }

    /// @notice Sets new master address
    /// @param _newMaster New master address
    function setMaster(address _newMaster) internal {
        bytes32 position = masterPosition;
        assembly {
            sstore(position, _newMaster)
        }
    }

    /// @notice Transfer mastership of the contract to new master
    /// @param _newMaster New master address
    function transferMastership(address _newMaster) external {
        requireMaster(msg.sender);
        require(
            _newMaster != address(0),
            "otp11"
        ); // otp11 - new master can't be zero address
        setMaster(_newMaster);
    }
}
