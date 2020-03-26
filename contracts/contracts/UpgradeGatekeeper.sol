pragma solidity 0.5.16;

import "./Events.sol";
import "./Ownable.sol";
import "./Bytes.sol";


/// @title Interface of the main contract
interface MainContract {

    /// @notice Notice period before activation preparation status of upgrade mode
    function upgradeNoticePeriod() external pure returns (uint);

    /// @notice Notifies proxy contract that notice period started
    function upgradeNoticePeriodStarted() external;

    /// @notice Notifies proxy contract that upgrade preparation status is activated
    function upgradePreparationStarted() external;

    /// @notice Notifies proxy contract that upgrade canceled
    function upgradeCanceled() external;

    /// @notice Notifies proxy contract that upgrade finishes
    function upgradeFinishes() external;

    /// @notice Checks that contract is ready for upgrade
    /// @return bool flag indicating that contract is ready for upgrade
    function readyForUpgrade() external view returns (bool);

}

/// @title Interface of the proxy contract
interface UpgradeableProxy {

    /// @notice Upgrades target of upgradeable contract
    /// @param newTarget New target
    /// @param newTargetInitializationParameters New target initialization parameters
    function upgradeTarget(address newTarget, bytes calldata newTargetInitializationParameters) external;

}

/// @title Upgrade Gatekeeper Contract
/// @author Matter Labs
contract UpgradeGatekeeper is UpgradeEvents, Ownable {

    /// @notice Array of addresses of proxy contracts managed by the gatekeeper
    address[] public proxies;

    /// @notice Upgrade mode statuses
    enum UpgradeStatus {
        Idle,
        NoticePeriod,
        Preparation
    }

    UpgradeStatus upgradeStatus;

    /// @notice Notice period activation timestamp (as seconds since unix epoch)
    /// @dev Will be equal to zero in case of not active upgrade mode
    uint noticePeriodActivationTime;

    /// @notice Addresses of the next versions of the contracts to be upgraded (if element of this array is equal to zero address it means that this proxy will not be upgraded)
    /// @dev Will be empty in case of not active upgrade mode
    address[] nextTargets;

    /// @notice Contract which allows finish upgrade during preparation status of upgrade
    MainContract mainContract;

    /// @notice Contract constructor
    /// @param _mainContractAddress Address of contract which processes priority operations
    /// @dev Calls Ownable contract constructor and adds _mainContractAddress to the list of contracts managed by the gatekeeper
    constructor(address _mainContractAddress) Ownable(msg.sender) public {
        mainContract = MainContract(_mainContractAddress);
    }

    /// @notice Adds a new proxy to the list of contracts managed by the gatekeeper
    /// @param proxy Address of proxy to add
    function addProxyContract(address proxy) external {
        requireMaster(msg.sender);
        require(upgradeStatus == UpgradeStatus.Idle, "apc11"); /// apc11 - proxy can't be added during upgrade

        proxies.push(proxy);
        emit ProxyAdded(proxy);
    }

    /// @notice Starts upgrade (activates notice period)
    /// @param newTargets New proxies targets (if element of this array is equal to zero address it means that this proxy will not be upgraded)
    function startUpgrade(address[] calldata newTargets) external {
        requireMaster(msg.sender);
        require(upgradeStatus == UpgradeStatus.Idle, "spu11"); // spu11 - unable to activate active upgrade mode
        require(newTargets.length == proxies.length, "spu12"); // spu12 - number of new targets must be equal to the number of proxies

        mainContract.upgradeNoticePeriodStarted();
        upgradeStatus = UpgradeStatus.NoticePeriod;
        noticePeriodActivationTime = now;
        nextTargets = newTargets;
        emit NoticePeriodStarted(newTargets);
    }

    /// @notice Cancels upgrade
    function cancelUpgrade() external {
        requireMaster(msg.sender);
        require(upgradeStatus != UpgradeStatus.Idle, "cpu11"); // cpu11 - unable to cancel not active upgrade mode

        mainContract.upgradeCanceled();
        upgradeStatus = UpgradeStatus.Idle;
        noticePeriodActivationTime = 0;
        delete nextTargets;
        emit UpgradeCanceled();
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
            emit PreparationStarted();
            return true;
        } else {
            return false;
        }
    }

    /// @notice Finishes upgrade
    /// @param initParametersConcatenated New targets initialization parameters per each proxy (concatenated into one array)
    /// @param sizeOfInitParameters Sizes of targets initialization parameters (in bytes)
    function finishUpgrade(bytes calldata initParametersConcatenated, uint[] calldata sizeOfInitParameters) external {
        requireMaster(msg.sender);
        require(upgradeStatus == UpgradeStatus.Preparation, "fpu11"); // fpu11 - unable to finish upgrade without preparation status active
        require(sizeOfInitParameters.length == proxies.length, "fpu12"); // fpu12 - number of new targets initialization parameters must be equal to the number of proxies
        require(mainContract.readyForUpgrade(), "fpu13"); // fpu13 - main contract is not ready for upgrade
        mainContract.upgradeFinishes();

        bytes memory initParametersConcatenated = initParametersConcatenated;
        uint processedBytes = 0;
        for (uint64 i = 0; i < proxies.length; i++) {
            address proxy = proxies[i];
            address nextTarget = nextTargets[i];
            if (nextTargets[i] == address(0)) {
                require(sizeOfInitParameters[i] == 0, "fpu14"); // fpu14 - there must be no init parameters bytes for proxy that wouldn't be upgraded
            } else {
                bytes memory targetInitParameters;
                (processedBytes, targetInitParameters) = Bytes.read(initParametersConcatenated, processedBytes, sizeOfInitParameters[i]);
                UpgradeableProxy(proxy).upgradeTarget(nextTarget, targetInitParameters);
                emit UpgradeCompleted(proxy, nextTarget);
            }
        }
        require(processedBytes == initParametersConcatenated.length, "fpu15"); // fpu15 - all targets initialization parameters bytes must be processed

        upgradeStatus = UpgradeStatus.Idle;
        noticePeriodActivationTime = 0;
        delete nextTargets;
    }

}
