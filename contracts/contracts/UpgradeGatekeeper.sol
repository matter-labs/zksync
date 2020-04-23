pragma solidity 0.5.16;
pragma experimental ABIEncoderV2;

import "./Events.sol";
import "./Ownable.sol";
import "./Upgradeable.sol";


/// @title Upgrade Gatekeeper Contract
/// @author Matter Labs
contract UpgradeGatekeeper is UpgradeEvents, Ownable {

    /// @notice Array of addresses of upgradeable contracts managed by the gatekeeper
    Upgradeable[] public managedContracts;

    /// @notice Upgrade mode statuses
    enum UpgradeStatus {
        Idle,
        NoticePeriod,
        Preparation
    }

    UpgradeStatus public upgradeStatus;

    /// @notice Notice period activation timestamp (as seconds since unix epoch)
    /// @dev Will be equal to zero in case of not active upgrade mode
    uint public noticePeriodActivationTime;

    /// @notice Addresses of the next versions of the contracts to be upgraded (if element of this array is equal to zero address it means that appropriate upgradeable contract wouldn't be upgraded this time)
    /// @dev Will be empty in case of not active upgrade mode
    address[] public nextTargets;

    /// @notice Contract which defines notice period duration and allows finish upgrade during preparation of it
    UpgradeableMaster public mainContract;

    /// @notice Contract constructor
    /// @param _mainContract Contract which defines notice period duration and allows finish upgrade during preparation of it
    /// @dev Calls Ownable contract constructor
    constructor(UpgradeableMaster _mainContract) Ownable(msg.sender) public {
        mainContract = _mainContract;
    }

    /// @notice Adds a new upgradeable contract to the list of contracts managed by the gatekeeper
    /// @param addr Address of upgradeable contract to add
    function addUpgradeable(address addr) external {
        requireMaster(msg.sender);
        require(upgradeStatus == UpgradeStatus.Idle, "apc11"); /// apc11 - upgradeable contract can't be added during upgrade

        managedContracts.push(Upgradeable(addr));
        emit UpgradeableAdd(Upgradeable(addr));
    }

    /// @notice Starts upgrade (activates notice period)
    /// @param newTargets New managed contracts targets (if element of this array is equal to zero address it means that appropriate upgradeable contract wouldn't be upgraded this time)
    function startUpgrade(address[] calldata newTargets) external {
        requireMaster(msg.sender);
        require(upgradeStatus == UpgradeStatus.Idle, "spu11"); // spu11 - unable to activate active upgrade mode
        require(newTargets.length == managedContracts.length, "spu12"); // spu12 - number of new targets must be equal to the number of managed contracts

        mainContract.upgradeNoticePeriodStarted();
        upgradeStatus = UpgradeStatus.NoticePeriod;
        noticePeriodActivationTime = now;
        nextTargets = newTargets;
        emit NoticePeriodStart(newTargets);
    }

    /// @notice Cancels upgrade
    function cancelUpgrade() external {
        requireMaster(msg.sender);
        require(upgradeStatus != UpgradeStatus.Idle, "cpu11"); // cpu11 - unable to cancel not active upgrade mode

        mainContract.upgradeCanceled();
        upgradeStatus = UpgradeStatus.Idle;
        noticePeriodActivationTime = 0;
        delete nextTargets;
        emit UpgradeCancel();
    }

    /// @notice Checks that preparation status is active and activates it if needed
    /// @return Bool flag indicating that preparation status is active after this call
    function startPreparation() public returns (bool) {
        require(upgradeStatus != UpgradeStatus.Idle, "ugp11"); // ugp11 - unable to activate preparation status in case of not active upgrade mode

        if (upgradeStatus == UpgradeStatus.Preparation) {
            return true;
        }

        if (now >= noticePeriodActivationTime + mainContract.upgradeNoticePeriod()) {
            upgradeStatus = UpgradeStatus.Preparation;
            mainContract.upgradePreparationStarted();
            emit PreparationStart();
            return true;
        } else {
            return false;
        }
    }

    /// @notice Finishes upgrade
    /// @param targetsInitializationParameters New targets initialization parameters per each upgradeable contract
    function finishUpgrade(bytes[] calldata targetsInitializationParameters) external {
        requireMaster(msg.sender);
        require(upgradeStatus == UpgradeStatus.Preparation, "fpu11"); // fpu11 - unable to finish upgrade without preparation status active
        require(targetsInitializationParameters.length == managedContracts.length, "fpu12"); // fpu12 - number of new targets initialization parameters must be equal to the number of managed contracts
        require(mainContract.readyForUpgrade(), "fpu13"); // fpu13 - main contract is not ready for upgrade
        mainContract.upgradeFinishes();

        for (uint64 i = 0; i < managedContracts.length; i++) {
            address newTarget = nextTargets[i];
            if (newTarget != address(0)) {
                managedContracts[i].upgradeTarget(newTarget, targetsInitializationParameters[i]);
                emit UpgradeComplete(managedContracts[i], newTarget);
            }
        }

        upgradeStatus = UpgradeStatus.Idle;
        noticePeriodActivationTime = 0;
        delete nextTargets;
    }

}
