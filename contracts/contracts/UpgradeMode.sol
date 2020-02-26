pragma solidity 0.5.16;

import "./Events.sol";
import "./Ownable.sol";


/// @title UpgradeMode Contract
/// @author Matter Labs
contract UpgradeMode is UpgradeModeEvents, Ownable {

    /// @notice Maximal upgrade time (in seconds)
    /// @dev After this period from the start of the upgrade anyone can cancel it forcibly
    uint256 constant MAX_UPGRADE_PERIOD = 60 * 60 * 24 * 14; /// 14 days

    /// @notice Waiting period to activate closed status mode (in seconds)
    uint256 constant WAIT_UPGRADE_MODE_PERIOD = 60 * 60 * 24 * 10; /// 10 days

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
        version = 1;
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

    /// @notice Force upgrade cancellation
    function forceCancel() external {
        requireMaster(msg.sender);
        require(
            waitUpgradeModeActive,
            "ufc11"
        ); // ufc11 - unable to cancel not active mode
        require(
            now >= activationTime + MAX_UPGRADE_PERIOD,
            "ufc12"
        ); // ufc12 - unable to force cancel upgrade until MAX_UPGRADE_PERIOD passes

        waitUpgradeModeActive = false;
        closedStatusActive = false;
        activationTime = 0;
        emit UpgradeForciblyCanceled(version);
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
            isClosedStatusActive(),
            "umf11"
        ); // umf11 - unable to finish upgrade without closed status active

        waitUpgradeModeActive = false;
        closedStatusActive = false;
        activationTime = 0;
        emit UpgradeCompleted(version);
        version++;
    }

}
