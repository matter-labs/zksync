pragma solidity 0.5.16;

import "./Ownable.sol";


/// @title Proxy Contract
/// @author Matter Labs
contract Proxy is Ownable {

    /// @notice Storage position of "target" (actual implementation address)
    bytes32 private constant targetPosition = keccak256("target");

    /// @notice Contract constructor
    /// @dev Calls Ownable contract constructor and initialize target
    /// @param target Initial implementation address
    /// @param targetInitializationParameters Target initialization parameters
    constructor(address target, bytes memory targetInitializationParameters) Ownable(msg.sender) public {
        setTarget(target);
        (bool initializationSuccess, ) = getTarget().delegatecall(
            abi.encodeWithSignature("initialize(bytes)", targetInitializationParameters)
        );
        require(initializationSuccess, "uin11"); // uin11 - target initialization failed
    }

    /// @notice Intercepts initialization calls
    function initialize(bytes calldata) external pure {
        revert("ini11"); // ini11 - interception of initialization call
    }

    /// @notice Returns target of contract
    /// @return Actual implementation address
    function getTarget() public view returns (address target) {
        bytes32 position = targetPosition;
        assembly {
            target := sload(position)
        }
    }

    /// @notice Sets new target of contract
    /// @param _newTarget New actual implementation address
    function setTarget(address _newTarget) internal {
        bytes32 position = targetPosition;
        assembly {
            sstore(position, _newTarget)
        }
    }

    /// @notice Upgrades target
    /// @param newTarget New target
    /// @param newTargetInitializationParameters New target initialization parameters
    function upgradeTarget(address newTarget, bytes calldata newTargetInitializationParameters) external {
        requireMaster(msg.sender);

        setTarget(newTarget);
        (bool initializationSuccess, ) = getTarget().delegatecall(
            abi.encodeWithSignature("initialize(bytes)", newTargetInitializationParameters)
        );
        require(initializationSuccess, "ufu11"); // ufu11 - target initialization failed
    }

    /// @notice Notifies proxy contract that notice period started
    function upgradeNoticePeriodStarted() external {
        requireMaster(msg.sender);
        getTarget().delegatecall(abi.encodeWithSignature("upgradeNoticePeriodStarted()"));
    }

    /// @notice Notifies proxy contract that upgrade preparation status is activated
    function upgradePreparationStarted() external {
        requireMaster(msg.sender);
        getTarget().delegatecall(abi.encodeWithSignature("upgradePreparationStarted()"));
    }

    /// @notice Notifies proxy contract that upgrade canceled
    function upgradeCanceled() external {
        requireMaster(msg.sender);
        getTarget().delegatecall(abi.encodeWithSignature("upgradeCanceled()"));
    }

    /// @notice Notifies proxy contract that upgrade finishes
    function upgradeFinishes() external {
        requireMaster(msg.sender);
        getTarget().delegatecall(abi.encodeWithSignature("upgradeFinishes()"));
    }

    /// @notice Performs a delegatecall to the contract implementation
    /// @dev Fallback function allowing to perform a delegatecall to the given implementation
    /// This function will return whatever the implementation call returns
    function() external payable {
        require(msg.data.length > 0, "pfb11"); // pfb11 - calldata must not be empty

        address _target = getTarget();
        assembly {
            // The pointer to the free memory slot
            let ptr := mload(0x40)
            // Copy function signature and arguments from calldata at zero position into memory at pointer position
            calldatacopy(ptr, 0x0, calldatasize)
            // Delegatecall method of the implementation contract, returns 0 on error
            let result := delegatecall(
                gas,
                _target,
                ptr,
                calldatasize,
                0x0,
                0
            )
            // Get the size of the last return data
            let size := returndatasize
            // Copy the size length of bytes from return data at zero position to pointer position
            returndatacopy(ptr, 0x0, size)
            // Depending on result value
            switch result
            case 0 {
                // End execution and revert state changes
                revert(ptr, size)
            }
            default {
                // Return data with length of size at pointers position
                return(ptr, size)
            }
        }
    }

}
