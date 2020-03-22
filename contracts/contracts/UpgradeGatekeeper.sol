pragma solidity 0.5.16;

import "./Events.sol";
import "./Ownable.sol";


/// @title Upgrade Gatekeeper Contract
/// @author Matter Labs
contract UpgradeGatekeeper is UpgradeEvents, Ownable {

    /// @notice Notice period before activation cleaning up status of upgrade mode (in seconds)
    uint256 constant NOTICE_PERIOD = 2 weeks;

    /// @notice Versions of proxy contracts
    mapping(address => uint64) public version;

    /// @notice Contract which processes priority operations
    address public mainContractAddress;

    /// @notice Upgrade mode statuses
    enum UpgradeStatus {
        Idle,
        NoticePeriod,
        CleaningUp
    }

    /// @notice Info for upgrade proxy
    struct UpgradeInfo {
        UpgradeStatus upgradeStatus;

        /// @notice Time of activating notice period
        /// @dev Will be equal to zero in case of not active mode
        uint256 activationTime;

        /// @notice Next target
        /// @dev Will store zero in case of not active upgrade mode
        address nextTarget;

        /// @notice Number of priority operations that must be verified at the time of finishing upgrade
        /// @dev Will store zero in case of not active cleaning up status of upgrade mode
        uint64 priorityOperationsToProcessBeforeUpgrade;
    }

    /// @notice UpgradeInfo per each proxy
    mapping(address => UpgradeInfo) public upgradeInfo;

    /// @notice Contract constructor
    /// @param _mainContractAddress Address of contract which processes priority operations
    /// @dev Calls Ownable contract constructor
    constructor(address _mainContractAddress) Ownable(msg.sender) public {
        mainContractAddress = _mainContractAddress;
    }

    /// @notice Activates notice period
    /// @param proxyAddress Address of proxy to process
    /// @param newTarget New target
    function upgradeProxy(address proxyAddress, address newTarget) external {
        requireMaster(msg.sender);
        require(upgradeInfo[proxyAddress].upgradeStatus == UpgradeGatekeeper.UpgradeStatus.Idle, "upa11"); // upa11 - unable to activate active upgrade mode

        upgradeInfo[proxyAddress].upgradeStatus = UpgradeGatekeeper.UpgradeStatus.NoticePeriod;
        upgradeInfo[proxyAddress].activationTime = now;
        upgradeInfo[proxyAddress].nextTarget = newTarget;
        upgradeInfo[proxyAddress].priorityOperationsToProcessBeforeUpgrade = 0;

        emit UpgradeModeActivated(proxyAddress, version[proxyAddress]);
    }

    /// @notice Cancels upgrade
    /// @param proxyAddress Address of proxy to process
    function cancelProxyUpgrade(address proxyAddress) external {
        requireMaster(msg.sender);
        require(upgradeInfo[proxyAddress].upgradeStatus != UpgradeGatekeeper.UpgradeStatus.Idle, "umc11"); // umc11 - unable to cancel not active upgrade mode

        upgradeInfo[proxyAddress].upgradeStatus = UpgradeGatekeeper.UpgradeStatus.Idle;
        upgradeInfo[proxyAddress].activationTime = 0;
        upgradeInfo[proxyAddress].nextTarget = address(0);
        upgradeInfo[proxyAddress].priorityOperationsToProcessBeforeUpgrade = 0;

        emit UpgradeCanceled(proxyAddress, version[proxyAddress]);
    }

    /// @notice Checks that cleaning up status is active and activates it if needed
    /// @param proxyAddress Address of proxy to process
    /// @return Bool flag indicating that cleaning up status is active after this call
    function activateCleaningUpStatusOfUpgrade(address proxyAddress) public returns (bool) {
        require(upgradeInfo[proxyAddress].upgradeStatus != UpgradeGatekeeper.UpgradeStatus.Idle, "uaf11"); // uaf11 - unable to activate cleaning up status in case of not active upgrade mode

        if (upgradeInfo[proxyAddress].upgradeStatus == UpgradeGatekeeper.UpgradeStatus.CleaningUp) {
            return true;
        }

        if (now >= upgradeInfo[proxyAddress].activationTime + NOTICE_PERIOD) {
            upgradeInfo[proxyAddress].upgradeStatus = UpgradeGatekeeper.UpgradeStatus.CleaningUp;

            (bool mainContractCallSuccess, bytes memory encodedResult) = mainContractAddress.staticcall(
                abi.encodeWithSignature("totalRegisteredPriorityOperations()")
            );
            require(mainContractCallSuccess, "uaf12"); // uaf12 - main contract static call failed
            uint64 totalRegisteredPriorityOperations = abi.decode(encodedResult, (uint64));
            upgradeInfo[proxyAddress].priorityOperationsToProcessBeforeUpgrade = totalRegisteredPriorityOperations;

            emit UpgradeModeCleaningUpStatusActivated(proxyAddress, version[proxyAddress]);
            return true;
        } else {
            return false;
        }
    }

    /// @notice Finishes upgrade
    /// @param proxyAddress Address of proxy to process
    /// @param newTargetInitializationParameters New target initialization parameters
    function finishProxyUpgrade(address proxyAddress, bytes calldata newTargetInitializationParameters) external {
        requireMaster(msg.sender);
        require(upgradeInfo[proxyAddress].upgradeStatus == UpgradeGatekeeper.UpgradeStatus.CleaningUp, "umf11"); // umf11 - unable to finish upgrade without cleaning up status active

        (bool mainContractCallSuccess, bytes memory encodedResult) = mainContractAddress.staticcall(
            abi.encodeWithSignature("verifiedPriorityOperations()")
        );
        require(mainContractCallSuccess, "umf12"); // umf12 - main contract static call failed
        uint64 verifiedPriorityOperations = abi.decode(encodedResult, (uint64));

        require(verifiedPriorityOperations >= upgradeInfo[proxyAddress].priorityOperationsToProcessBeforeUpgrade, "umf13"); // umf13 - can't finish upgrade before verifing all priority operations received before start of cleaning up status

        (bool proxyUpgradeCallSuccess, ) = proxyAddress.call(
            abi.encodeWithSignature("upgradeTarget(address,bytes)", upgradeInfo[proxyAddress].nextTarget, newTargetInitializationParameters)
        );
        require(proxyUpgradeCallSuccess, "umf14"); // umf14 - proxy contract call failed

        emit UpgradeCompleted(proxyAddress, version[proxyAddress], upgradeInfo[proxyAddress].nextTarget);
        version[proxyAddress]++;

        upgradeInfo[proxyAddress].upgradeStatus = UpgradeGatekeeper.UpgradeStatus.Idle;
        upgradeInfo[proxyAddress].activationTime = 0;
        upgradeInfo[proxyAddress].nextTarget = address(0);
        upgradeInfo[proxyAddress].priorityOperationsToProcessBeforeUpgrade = 0;
    }

}
