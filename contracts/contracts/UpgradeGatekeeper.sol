pragma solidity 0.5.16;

import "./Events.sol";
import "./Ownable.sol";
import "./Bytes.sol";


/// @title Upgrade Gatekeeper Contract
/// @author Matter Labs
contract UpgradeGatekeeper is UpgradeEvents, Ownable {

    /// @notice Notice period before activation preparation status of upgrade mode (in seconds)
    uint constant NOTICE_PERIOD = 2 weeks;

    /// @notice Versions of proxy contracts
    mapping(address => uint64) public version;

    /// @notice Contract which processes priority operations
    address public mainContractAddress;

    /// @notice Number of proxy contracts managed by the gatekeeper
    uint64 public numberOfProxies;

    /// @notice Addresses of proxy contracts managed by the gatekeeper
    mapping(uint64 => address) public proxyAddress;

    /// @notice Upgrade mode statuses
    enum UpgradeStatus {
        Idle,
        NoticePeriod,
        Preparation
    }

    UpgradeStatus upgradeStatus;

    /// @notice Notice period activation timestamp (in seconds)
    /// @dev Will be equal to zero in case of not active mode
    uint activationTime;

    /// @notice Address of the next version of the contract to be upgraded per each proxy
    /// @dev Will store zero in case of not active upgrade mode
    mapping(address => address) nextTarget;

    /// @notice Number of priority operations that must be verified by main contract at the time of finishing upgrade
    /// @dev Will store zero in case of not active upgrade mode or not active preparation status of upgrade mode
    uint64 priorityOperationsToProcessBeforeUpgrade;

    /// @notice Contract constructor
    /// @param _mainContractAddress Address of contract which processes priority operations
    /// @dev Calls Ownable contract constructor
    constructor(address _mainContractAddress) Ownable(msg.sender) public {
        mainContractAddress = _mainContractAddress;
    }

    /// @notice Clears list of proxies managed by the gatekeeper (for case of mistake when adding new proxies to the gatekeeper)
    function clearProxyList() external {
        requireMaster(msg.sender);

        upgradeStatus = UpgradeGatekeeper.UpgradeStatus.Idle;
        activationTime = 0;
        for (uint64 i = 0; i < numberOfProxies; i++) {
            address proxy = proxyAddress[i];
            nextTarget[proxy] = address(0);
        }
        priorityOperationsToProcessBeforeUpgrade = 0;

        numberOfProxies = 0;
        emit ProxyListCleared();
    }

    /// @notice Adds a new proxy to the list of contracts managed by the gatekeeper
    /// @param proxy Address of proxy to add
    function addProxyContract(address proxy) external {
        requireMaster(msg.sender);
        require(upgradeStatus == UpgradeGatekeeper.UpgradeStatus.Idle, "apc11"); /// apc11 - proxy can't be added during upgrade

        proxyAddress[numberOfProxies] = proxy;
        numberOfProxies++;

        emit ProxyAdded(proxy);
    }

    /// @notice Starts upgrade (activates notice period)
    /// @param newTargets New proxies targets
    function startProxyUpgrade(address[] calldata newTargets) external {
        requireMaster(msg.sender);
        require(upgradeStatus == UpgradeGatekeeper.UpgradeStatus.Idle, "spu11"); // spu11 - unable to activate active upgrade mode
        require(newTargets.length == numberOfProxies, "spu12"); // spu12 - number of new targets must be equal to the number of proxies

        upgradeStatus = UpgradeGatekeeper.UpgradeStatus.NoticePeriod;
        activationTime = now;
        for (uint64 i = 0; i < numberOfProxies; i++) {
            address proxy = proxyAddress[i];
            nextTarget[proxy] = newTargets[i];
        }
        priorityOperationsToProcessBeforeUpgrade = 0;

        emit UpgradeModeActivated();
    }

    /// @notice Cancels upgrade
    function cancelProxyUpgrade() external {
        requireMaster(msg.sender);
        require(upgradeStatus != UpgradeGatekeeper.UpgradeStatus.Idle, "cpu11"); // cpu11 - unable to cancel not active upgrade mode

        upgradeStatus = UpgradeGatekeeper.UpgradeStatus.Idle;
        activationTime = 0;
        for (uint64 i = 0; i < numberOfProxies; i++) {
            address proxy = proxyAddress[i];
            nextTarget[proxy] = address(0);
        }
        priorityOperationsToProcessBeforeUpgrade = 0;

        emit UpgradeCanceled();
    }

    /// @notice Checks that preparation status is active and activates it if needed
    /// @return Bool flag indicating that preparation status is active after this call
    function startPreparation() public returns (bool) {
        require(upgradeStatus != UpgradeGatekeeper.UpgradeStatus.Idle, "ugp11"); // ugp11 - unable to activate preparation status in case of not active upgrade mode

        if (upgradeStatus == UpgradeGatekeeper.UpgradeStatus.Preparation) {
            return true;
        }

        if (now >= activationTime + NOTICE_PERIOD) {
            upgradeStatus = UpgradeGatekeeper.UpgradeStatus.Preparation;

            (bool mainContractCallSuccess, bytes memory encodedResult) = mainContractAddress.staticcall(
                abi.encodeWithSignature("totalRegisteredPriorityOperations()")
            );
            require(mainContractCallSuccess, "ugp12"); // ugp12 - main contract static call failed
            uint64 totalRegisteredPriorityOperations = abi.decode(encodedResult, (uint64));
            priorityOperationsToProcessBeforeUpgrade = totalRegisteredPriorityOperations;

            emit UpgradeModePreparationStatusActivated();
            return true;
        } else {
            return false;
        }
    }

    /// @notice Finishes upgrade
    /// @param initParametersConcatenated New targets initialization parameters per each proxy (concatenated into one array)
    /// @param sizeOfInitParameters Sizes of targets initialization parameters (in bytes)
    function finishProxyUpgrade(bytes calldata initParametersConcatenated, uint[] calldata sizeOfInitParameters) external {
        requireMaster(msg.sender);
        require(upgradeStatus == UpgradeGatekeeper.UpgradeStatus.Preparation, "fpu11"); // fpu11 - unable to finish upgrade without preparation status active
        require(sizeOfInitParameters.length == numberOfProxies, "fpu12"); // fpu12 - number of new targets initialization parameters must be equal to the number of proxies

        (bool mainContractCallSuccess, bytes memory encodedResult) = mainContractAddress.staticcall(
            abi.encodeWithSignature("totalVerifiedPriorityOperations()")
        );
        require(mainContractCallSuccess, "fpu13"); // fpu13 - main contract static call failed
        uint64 totalVerifiedPriorityOperations = abi.decode(encodedResult, (uint64));

        require(totalVerifiedPriorityOperations >= priorityOperationsToProcessBeforeUpgrade, "fpu14"); // fpu14 - can't finish upgrade before verifying all priority operations received before start of preparation status

        bytes memory initParametersConcatenated = initParametersConcatenated;
        uint processedBytes = 0;
        for (uint64 i = 0; i < numberOfProxies; i++) {
            address proxy = proxyAddress[i];
            bytes memory targetInitParameters;

            // TODO: remove this when Bytes.slice function will be fixed
            if (sizeOfInitParameters[i] == 0){
                targetInitParameters = new bytes(0);
            } else {
                (processedBytes, targetInitParameters) = Bytes.read(initParametersConcatenated, processedBytes, sizeOfInitParameters[i]);
            }

            (bool proxyUpgradeCallSuccess, ) = proxy.call(
                abi.encodeWithSignature("upgradeTarget(address,bytes)", nextTarget[proxy], targetInitParameters)
            );
            require(proxyUpgradeCallSuccess, "fpu15"); // fpu15 - proxy contract call failed

            emit UpgradeCompleted(proxy, version[proxy], nextTarget[proxy]);
            version[proxy]++;
        }
        require(processedBytes == initParametersConcatenated.length, "fpu16"); // fpu16 - all targets initialization parameters bytes must be processed

        upgradeStatus = UpgradeGatekeeper.UpgradeStatus.Idle;
        activationTime = 0;
        for (uint64 i = 0; i < numberOfProxies; i++) {
            address proxy = proxyAddress[i];
            nextTarget[proxy] = address(0);
        }
        priorityOperationsToProcessBeforeUpgrade = 0;
    }

}
