pragma solidity 0.5.16;

import "./Events.sol";
import "./Ownable.sol";
import "./Proxy.sol";


/// @title Upgrade Module Contract
/// @author Matter Labs
contract UpgradeModule is UpgradeEvents, Ownable {

    /// @notice Waiting period to activate finalize status mode (in seconds)
    uint256 constant WAIT_UPGRADE_MODE_PERIOD = 2 weeks;

    /// @notice Versions of proxy contracts
    mapping(address => uint64) public version;

    /// @notice Contract which processes priority operations
    address public mainContractAddress;

    /// @notice Upgrade mode statuses
    enum UpgradeStatus {
        NotActive,
        WaitUpgrade,
        Finalize
    }

    /// @notice Info for upgrade proxy
    struct UpgradeInfo {
        UpgradeStatus upgradeStatus;

        /// @notice Time of activating waiting upgrade mode
        /// @dev Will be equal to zero in case of not active mode
        uint256 activationTime;

        /// @notice Next target
        /// @dev Will store zero in case of not active upgrade mode
        address nextTarget;

        /// @notice Number of priority requests that must be verified at the time of finishing upgrade
        /// @dev Will store zero in case of not active finalize status of upgrade mode
        uint64 priorityRequestsToProcessBeforeUpgrade;
    }

    /// @notice UpgradeInfo per each proxy
    mapping(address => UpgradeInfo) public upgradeInfo;

    /// @notice Contract constructor
    /// @param _mainContractAddress Address of contract which processes priority operations
    /// @dev Calls Ownable contract constructor
    constructor(address _mainContractAddress) Ownable() public {
        mainContractAddress = _mainContractAddress;
    }

    /// @notice Activates wait upgrade status
    /// @param proxyAddress Address of proxy to process
    /// @param newTarget New target
    function upgradeProxy(address proxyAddress, address newTarget) external {
        requireMaster(msg.sender);
        require(
            upgradeInfo[proxyAddress].upgradeStatus == UpgradeModule.UpgradeStatus.NotActive,
            "upa11"
        ); // upa11 - unable to activate active upgrade mode

        Proxy(address(uint160(proxyAddress))).upgradeTarget(newTarget);

        upgradeInfo[proxyAddress].upgradeStatus = UpgradeModule.UpgradeStatus.WaitUpgrade;
        upgradeInfo[proxyAddress].activationTime = now;
        upgradeInfo[proxyAddress].nextTarget = newTarget;
        upgradeInfo[proxyAddress].priorityRequestsToProcessBeforeUpgrade = 0;

        emit UpgradeModeActivated(proxyAddress, version[proxyAddress]);
    }

    /// @notice Cancels upgrade
    /// @param proxyAddress Address of proxy to process
    function cancelProxyUpgrade(address proxyAddress) external {
        requireMaster(msg.sender);
        require(
            upgradeInfo[proxyAddress].upgradeStatus != UpgradeModule.UpgradeStatus.NotActive,
            "umc11"
        ); // umc11 - unable to cancel not active upgrade mode

        upgradeInfo[proxyAddress].upgradeStatus = UpgradeModule.UpgradeStatus.NotActive;
        upgradeInfo[proxyAddress].activationTime = 0;
        upgradeInfo[proxyAddress].nextTarget = address(0);
        upgradeInfo[proxyAddress].priorityRequestsToProcessBeforeUpgrade = 0;

        emit UpgradeCanceled(proxyAddress, version[proxyAddress]);
    }

    /// @notice Checks that finalize status is active and activates it if needed
    /// @param proxyAddress Address of proxy to process
    /// @return Bool flag indicating that finalize status is active after this call
    function activeFinalizeStatusOfUpgrade(address proxyAddress) public returns (bool) {
        require(
            upgradeInfo[proxyAddress].upgradeStatus != UpgradeModule.UpgradeStatus.NotActive,
            "uaf11"
        ); // uaf11 - unable to activate finalize status in case of not active upgrade mode

        if (upgradeInfo[proxyAddress].upgradeStatus == UpgradeModule.UpgradeStatus.Finalize) {
            return true;
        }

        if (now >= upgradeInfo[proxyAddress].activationTime + WAIT_UPGRADE_MODE_PERIOD) {
            upgradeInfo[proxyAddress].upgradeStatus = UpgradeModule.UpgradeStatus.Finalize;

            (bool callSuccess, bytes memory encodedResult) = mainContractAddress.staticcall(
                abi.encodeWithSignature("registeredPriorityOperations()")
            );
            require(
                callSuccess,
                "uaf12"
            ); // uaf12 - main contract static call failed
            uint64 registeredPriorityOperations = abi.decode(encodedResult, (uint64));
            upgradeInfo[proxyAddress].priorityRequestsToProcessBeforeUpgrade = registeredPriorityOperations;

            emit UpgradeModeFinalizeStatusActivated(proxyAddress, version[proxyAddress]);
            return true;
        }
        else{
            return false;
        }
    }

    /// @notice Finishes upgrade
    /// @param proxyAddress Address of proxy to process
    /// @param newTargetInitializationParameters New target initialization parameters
    function finishProxyUpgrade(address proxyAddress, bytes calldata newTargetInitializationParameters) external {
        requireMaster(msg.sender);
        require(
            upgradeInfo[proxyAddress].upgradeStatus == UpgradeModule.UpgradeStatus.Finalize,
            "umf11"
        ); // umf11 - unable to finish upgrade without finalize status active

        (bool callSuccess, bytes memory encodedResult) = mainContractAddress.staticcall(
            abi.encodeWithSignature("verifiedPriorityOperations()")
        );
        require(
            callSuccess,
            "umf12"
        ); // umf12 - main contract static call failed
        uint64 verifiedPriorityOperations = abi.decode(encodedResult, (uint64));

        require(
            verifiedPriorityOperations >= upgradeInfo[proxyAddress].priorityRequestsToProcessBeforeUpgrade,
            "umf13"
        ); // umf13 - can't finish upgrade before verifing all priority operations received before start of finalize status

        Proxy(address(uint160(proxyAddress))).finishTargetUpgrade(upgradeInfo[proxyAddress].nextTarget, newTargetInitializationParameters);

        emit UpgradeCompleted(proxyAddress, version[proxyAddress], upgradeInfo[proxyAddress].nextTarget);
        version[proxyAddress]++;

        upgradeInfo[proxyAddress].upgradeStatus = UpgradeModule.UpgradeStatus.NotActive;
        upgradeInfo[proxyAddress].activationTime = 0;
        upgradeInfo[proxyAddress].nextTarget = address(0);
        upgradeInfo[proxyAddress].priorityRequestsToProcessBeforeUpgrade = 0;
    }

}
