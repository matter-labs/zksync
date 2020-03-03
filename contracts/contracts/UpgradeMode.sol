pragma solidity 0.5.16;

import "./Events.sol";
import "./Ownable.sol";


/// @title UpgradeMode Contract
/// @author Matter Labs
contract UpgradeMode is UpgradeModeEvents, Ownable {

    /// @notice Maximal upgrade time (in seconds)
    /// @dev After this period from the start of the upgrade anyone can cancel it forcibly
    uint256 constant MAX_UPGRADE_PERIOD = 2 weeks;

    /// @notice Waiting period to activate finalize status mode (in seconds)
    uint256 constant WAIT_UPGRADE_MODE_PERIOD = 10 days;

    /// @notice Version of upgradeable field
    uint64 public version;

    /// @notice Flag indicating that wait upgrade mode is active
    bool public waitUpgradeModeActive;

    /// @notice Flag indicating that finalize status is active
    bool public finalizeStatusActive;

    /// @notice Time of activating waiting upgrade mode
    /// @dev Will be equal to zero in case of not active mode
    uint256 public activationTime;

    /// @notice Contract constructor
    /// @dev Calls Ownable contract constructor
    constructor() Ownable() public {
        version = 1;
        waitUpgradeModeActive = false;
        finalizeStatusActive = false;
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
        finalizeStatusActive = false;
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
        finalizeStatusActive = false;
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
        finalizeStatusActive = false;
        activationTime = 0;
        emit UpgradeForciblyCanceled(version);
    }

    /// @notice Checks that finalize status is active and activates it if needed
    /// @return Bool flag indicating that finalize status is active
    function isFinalizeStatusActive() public returns (bool) {
        if (!waitUpgradeModeActive) {
            return false;
        }
        if (finalizeStatusActive) {
            return true;
        }
        if (now >= activationTime + WAIT_UPGRADE_MODE_PERIOD) {
            finalizeStatusActive = true;
            emit UpgradeModeFinalizeStatusActivated(version);
        }
        return finalizeStatusActive;
    }

    /// @notice Finishes upgrade
    function finish() external {
        requireMaster(msg.sender);
        require(
            isFinalizeStatusActive(),
            "umf11"
        ); // umf11 - unable to finish upgrade without finalize status active

        waitUpgradeModeActive = false;
        finalizeStatusActive = false;
        activationTime = 0;
        emit UpgradeCompleted(version);
        version++;
    }

}
