pragma solidity 0.5.16;

import "./Ownable.sol";


/// @title Upgradeable contract
/// @author Matter Labs
contract Upgradeable is Ownable {

    /// @notice Storage position of "target" (actual implementation address)
    bytes32 private constant targetPosition = keccak256("target");

    /// @notice Contract constructor
    /// @dev Calls Ownable contract constructor
    constructor() Ownable(msg.sender) public {

    }

    /// @notice Intercepts initialization calls
    function initialize(bytes calldata) external pure {
        revert("ini11"); // ini11 - interception of initialization call
    }

    /// @notice Upgradeable contract initialization
    /// @param target Initial implementation address
    /// @param targetInitializationParameters Target initialization parameters
    function initializeTarget(address target, bytes calldata targetInitializationParameters) external {
        requireMaster(msg.sender);

        setTarget(target);
        (bool initializationSuccess, ) = getTarget().delegatecall(
            abi.encodeWithSignature("initialize(bytes)", targetInitializationParameters)
        );
        require(initializationSuccess, "uin11"); // uin11 - target initialization failed
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

}
