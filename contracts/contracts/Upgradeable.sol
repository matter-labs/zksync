pragma solidity 0.5.16;

import "./Ownable.sol";
import "./UpgradeMode.sol";


/// @title Upgradeable contract
/// @author Matter Labs
contract Upgradeable is Ownable {

    /// @notice Storage position of contract version index
    bytes32 private constant versionPosition = keccak256("version");

    /// @notice Storage position of "target" (actual implementation address)
    bytes32 private constant targetPosition = keccak256("target");

    /// @notice Storage position of next "target" (in case the contract is in status of waiting to upgrade)
    /// @dev Will store zero in case of not active upgrade mode
    bytes32 private constant nextTargetPosition = keccak256("nextTarget");

    /// @notice Storage position of UpgradeMode contract address
    bytes32 private constant upgradeModeAddressPosition = keccak256("UpgradeModeAddress");

    /// @notice Contract constructor
    /// @dev Calls Ownable contract constructor and creates UpgradeMode contract
    constructor() Ownable() public {
        setVersion(0);
        setTarget(address(0));
        setNextTarget(address(0));
        setUpgradeModeAddress(address(new UpgradeMode()));
    }

    /// @notice Upgradeable contract initialization
    /// @param _target Initial implementation address
    /// @param _targetInitializationParameters Target initialization parameters
    function initialize(address _target, bytes calldata _targetInitializationParameters) external {
        requireMaster(msg.sender);
        require(
            getVersion() == 0,
            "uin11"
        ); // uin11 - upgradeable contract already initialized

        setVersion(1);

        setTarget(_target);
        (bool initializationSuccess, ) = getTarget().delegatecall(
            abi.encodeWithSignature("initialize(address,bytes)", getUpgradeModeAddress(), _targetInitializationParameters)
        );
        require(
            initializationSuccess,
            "uin12"
        ); // uin12 - target initialization failed
    }

    /// @notice Returns contract version index
    /// @return Contract version index
    function getVersion() public view returns (uint64 version) {
        bytes32 position = versionPosition;
        assembly {
            version := sload(position)
        }
    }

    /// @notice Sets new contract version index
    /// @param _newVersion New contract version index
    function setVersion(uint64 _newVersion) internal {
        bytes32 position = versionPosition;
        assembly {
            sstore(position, _newVersion)
        }
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

    /// @notice Returns next target
    /// @return Next target address
    function getNextTarget() public view returns (address nextTarget) {
        bytes32 position = nextTargetPosition;
        assembly {
            nextTarget := sload(position)
        }
    }

    /// @notice Sets new next target
    /// @param _newNextTarget New next target value
    function setNextTarget(address _newNextTarget) internal {
        bytes32 position = nextTargetPosition;
        assembly {
            sstore(position, _newNextTarget)
        }
    }

    /// @notice Returns UpgradeMode contract address
    /// @return UpgradeMode contract address
    function getUpgradeModeAddress() public view returns (address upgradeModeAddress) {
        bytes32 position = upgradeModeAddressPosition;
        assembly {
            upgradeModeAddress := sload(position)
        }
    }

    /// @notice Sets new UpgradeMode contract address
    /// @param _newUpgradeModeAddress New UpgradeMode contract address
    function setUpgradeModeAddress(address _newUpgradeModeAddress) internal {
        bytes32 position = upgradeModeAddressPosition;
        assembly {
            sstore(position, _newUpgradeModeAddress)
        }
    }

    /// @notice Starts upgrade
    /// @param _newTarget Next actual implementation address
    function upgradeTarget(address _newTarget) external {
        requireMaster(msg.sender);
        require(
            _newTarget != address(0),
            "uut11"
        ); // uut11 - new actual implementation address can't be zero address
        require(
            getTarget() != _newTarget,
            "uut12"
        ); // uut12 - new actual implementation address can't be equal to previous

        UpgradeMode UpgradeMode = UpgradeMode(getUpgradeModeAddress());
        UpgradeMode.activate();

        setNextTarget(_newTarget);
    }

    /// @notice Cancels upgrade
    function cancelUpgradeTarget() external {
        requireMaster(msg.sender);

        UpgradeMode UpgradeMode = UpgradeMode(getUpgradeModeAddress());
        UpgradeMode.cancel();

        setNextTarget(address(0));
    }

    /// @notice Force upgrade cancellation
    function forceCancelUpgradeTarget() external {
        UpgradeMode UpgradeMode = UpgradeMode(getUpgradeModeAddress());
        UpgradeMode.forceCancel();

        setNextTarget(address(0));
    }

    /// @notice Checks that target is ready to be upgraded
    /// @return Bool flag indicating that target is ready to be upgraded
    function targetReadyToBeUpgraded() public returns (bool) {
        (bool success, bytes memory result) = getTarget().delegatecall(abi.encodeWithSignature("readyToBeUpgraded()"));
        require(
            success,
            "utr11"
        ); // utr11 - target readyToBeUpgraded() call failed

        return abi.decode(result, (bool));
    }

    /// @notice Finishes upgrade
    /// @param _newTargetInitializationParameters New target initialization parameters
    function finishTargetUpgrade(bytes calldata _newTargetInitializationParameters) external {
        requireMaster(msg.sender);
        require(
            targetReadyToBeUpgraded(),
            "ufu11"
        ); // ufu11 - target is not ready to be upgraded

        UpgradeMode UpgradeMode = UpgradeMode(getUpgradeModeAddress());
        UpgradeMode.finish();

        setVersion(getVersion() + 1);

        setTarget(getNextTarget());
        setNextTarget(address(0));

        (bool initializationSuccess, ) = getTarget().delegatecall(
            abi.encodeWithSignature("initialize(address,bytes)", getUpgradeModeAddress(), _newTargetInitializationParameters)
        );
        require(
            initializationSuccess,
            "ufu12"
        ); // ufu12 - target initialization failed
    }

}
