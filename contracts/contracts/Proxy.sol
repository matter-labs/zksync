pragma solidity ^0.5.0;

import "./Ownable.sol";
import "./Upgradeable.sol";


/// @title Proxy Contract
/// @dev NOTICE: Proxy must implement UpgradeableMaster interface to prevent calling some function of it not by master of proxy
/// @author Matter Labs
contract Proxy is Upgradeable, UpgradeableMaster, Ownable {

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

    /// UpgradeableMaster functions

    /// @notice Notice period before activation preparation status of upgrade mode
    function upgradeNoticePeriod() external returns (uint) {
        (bool success, bytes memory result) = getTarget().delegatecall(abi.encodeWithSignature("upgradeNoticePeriod()"));
        require(success, "unp11"); // unp11 - upgradeNoticePeriod delegatecall failed
        return abi.decode(result, (uint));
    }

    /// @notice Notifies proxy contract that notice period started
    function upgradeNoticePeriodStarted() external {
        requireMaster(msg.sender);
        (bool success, ) = getTarget().delegatecall(abi.encodeWithSignature("upgradeNoticePeriodStarted()"));
        require(success, "nps11"); // nps11 - upgradeNoticePeriodStarted delegatecall failed
    }

    /// @notice Notifies proxy contract that upgrade preparation status is activated
    function upgradePreparationStarted() external {
        requireMaster(msg.sender);
        (bool success, ) = getTarget().delegatecall(abi.encodeWithSignature("upgradePreparationStarted()"));
        require(success, "ups11"); // ups11 - upgradePreparationStarted delegatecall failed
    }

    /// @notice Notifies proxy contract that upgrade canceled
    function upgradeCanceled() external {
        requireMaster(msg.sender);
        (bool success, ) = getTarget().delegatecall(abi.encodeWithSignature("upgradeCanceled()"));
        require(success, "puc11"); // puc11 - upgradeCanceled delegatecall failed
    }

    /// @notice Notifies proxy contract that upgrade finishes
    function upgradeFinishes() external {
        requireMaster(msg.sender);
        (bool success, ) = getTarget().delegatecall(abi.encodeWithSignature("upgradeFinishes()"));
        require(success, "puf11"); // puf11 - upgradeFinishes delegatecall failed
    }

    /// @notice Checks that contract is ready for upgrade
    /// @return bool flag indicating that contract is ready for upgrade
    function readyForUpgrade() external returns (bool) {
        (bool success, bytes memory result) = getTarget().delegatecall(abi.encodeWithSignature("readyForUpgrade()"));
        require(success, "rfu11"); // rfu11 - readyForUpgrade delegatecall failed
        return abi.decode(result, (bool));
    }

}
