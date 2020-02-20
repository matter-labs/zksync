pragma solidity 0.5.16;

import "./Events.sol";
import "./Ownable.sol";


/// @title WaitUpgradeMode Contract
/// @author Matter Labs
contract WaitUpgradeMode is UpgradeModeEvents, Ownable {

    /// @notice Waiting period to activate closed status mode (in seconds)
    uint256 constant WAIT_UPGRADE_MODE_PERIOD = 60 * 60 * 24 * 7 * 2; /// two weeks

    /// @notice Version of upgradeable field
    uint64 public version;

    /// @notice Flag indicating that wait upgrade mode is active
    bool public waitUpgradeModeActive;

    /// @notice Flag indicating that closed status is active
    bool public closedStatusActive;

    /// @notice Time of activating waiting upgrade mode
    /// @dev Will be equal to zero in case of not active mode
    uint256 public activationTime;

    /// @notice Contract constructor
    /// @dev Calls Ownable contract constructor
    constructor() Ownable() public {
        version = 0;
        waitUpgradeModeActive = false;
        closedStatusActive = false;
        activationTime = 0;
    }

    /// @notice Activates wait upgrade mode
    function activate() external {
        requireMaster(msg.sender);
        require(
            !waitUpgradeModeActive,
            "uma11"
        ); // uma11 - unable to activate active mode

        waitUpgradeModeActive = true;
        closedStatusActive = false;
        activationTime = now;
        emit UpgradeModeActivated(version);
    }

    /// @notice Cancels upgrade
    function cancel() external {
        requireMaster(msg.sender);
        require(
            waitUpgradeModeActive,
            "umc11"
        ); // umc11 - unable to cancel not active mode

        waitUpgradeModeActive = false;
        closedStatusActive = false;
        activationTime = 0;
        emit UpgradeCanceled(version);
    }

    /// @notice Checks that closed status is active and activates it if needed
    /// @return Bool flag indicating that closed status is active
    function isClosedStatusActive() public returns (bool) {
        if (!waitUpgradeModeActive) {
            return false;
        }
        if (closedStatusActive) {
            return true;
        }
        if (now >= activationTime + WAIT_UPGRADE_MODE_PERIOD) {
            closedStatusActive = true;
            emit UpgradeModeClosedStatusActivated(version);
        }
        return closedStatusActive;
    }

    /// @notice Finishes upgrade
    function finish() external {
        requireMaster(msg.sender);
        require(
            closedStatusActive,
            "umf11"
        ); // umf11 - unable to finish upgrade without closed status active

        waitUpgradeModeActive = false;
        closedStatusActive = false;
        activationTime = 0;

        emit UpgradeCompleted(version);
        version++;
    }

}
