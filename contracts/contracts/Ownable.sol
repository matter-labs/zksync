pragma solidity 0.5.16;

/// @title Ownable Contract
/// @author Matter Labs
contract Ownable {
    /// @notice Storage position of the owner address
    bytes32 private constant ownerPosition = keccak256("owner");

    /// @notice Contract constructor
    /// @dev Sets msg sender address as owner address
    constructor() public {
        setOwner(msg.sender);
    }

    /// @notice Check if specified address is owner
    /// @param _address Address to check
    function requireOwner(address _address) internal view {
        require(
            _address == getOwner(),
            "oro11"
        ); // oro11 - only by owner
    }

    /// @notice Returns contract owner address
    /// @return Owner address
    function getOwner() public view returns (address owner) {
        bytes32 position = ownerPosition;
        assembly {
            owner := sload(position)
        }
    }

    /// @notice Sets new owner address
    /// @param _newOwner New owner address
    function setOwner(address _newOwner) internal {
        bytes32 position = ownerPosition;
        assembly {
            sstore(position, _newOwner)
        }
    }

    /// @notice Transfer ownership of the contract to new owner
    /// @param _newOwner New owner address
    function transferOwnership(address _newOwner) external {
        requireOwner(msg.sender);
        require(
            _newOwner != address(0),
            "otp11"
        ); // otp11 - new owner can't be zero address
        setOwner(_newOwner);
    }
}
