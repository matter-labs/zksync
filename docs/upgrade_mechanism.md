# Upgrade mechanism

This is an image of the dependencies structure.
Use it for a better understanding.
![](https://i.imgur.com/nXRKt8a.png)

## Contracts and interfaces

### Ownable

`Ownable` is a contract that stores the address of its **master** at the storage slot with index `keccak256("master")`.

### UpgradeableMaster

`UpgradeableMaster` is an interface of contract, which will decide transition between **phases of upgrade** (this will be explained below)

### Upgradeable

`Upgradeable` is an interface of contract, which knows its **target** (address of implementation). The target can be changed by calling `upgradeTarget` method.

```sol
/// @title Interface of the upgradeable contract
interface Upgradeable {

    /// @notice Upgrades target of upgradeable contract
    /// @param newTarget New target
    /// @param newTargetInitializationParameters New target initialization parameters
    function upgradeTarget(address newTarget, bytes calldata newTargetInitializationParameters) external;

}
```

### Proxy

`Proxy` is an interim contract between the caller and the real implementation of the contract.

```sol
/// @title Proxy Contract
contract Proxy is Upgradeable, UpgradeableMaster, Ownable
```

**Note: storage of this contract will be a context in which all processes of the target will work.** Proxy will store address of its **target** at the storage slot with index `keccak256("target")`.

As Proxy implements Upgradeable, it can change its target, but only `master` can do it.

Proxy have a fallback function:

```sol
/// @notice Performs a delegatecall to the contract implementation
/// @dev Fallback function allowing to perform a delegatecall to the given implementation
/// This function will return whatever the implementation call returns
function() external payable {
    require(msg.data.length > 0, "pfb11"); // pfb11 - calldata must not be empty

    address _target = getTarget();
    assembly {
        // The pointer to the free memory slot
        let ptr := mload(0x40)
        // Copy function signature and arguments from calldata at zero position into memory at pointer position
        calldatacopy(ptr, 0x0, calldatasize)
        // Delegatecall method of the implementation contract, returns 0 on error
        let result := delegatecall(
            gas,
            _target,
            ptr,
            calldatasize,
            0x0,
            0
        )
        // Get the size of the last return data
        let size := returndatasize
        // Copy the size length of bytes from return data at zero position to pointer position
        returndatacopy(ptr, 0x0, size)
        // Depending on result value
        switch result
        case 0 {
            // End execution and revert state changes
            revert(ptr, size)
        }
        default {
            // Return data with length of size at pointers position
            return(ptr, size)
        }
    }
}
```

There is some type of calls that Proxy must intercept without uncheck submitting to processing to a fallback function: calling the `initialize` function (in the right way this function will be called from proxy contract directly) and functions of `UpgradeableMaster` interface (that is a reason why Proxy implements it).

### UpgradeGatekeeper

`UpgradeGatekeeper` is a contract that will manage the upgrade process. It is needed to prevent upgrading rollup contracts by the master in one function call.

## Deploying and upgrade process

### Deploying

When a target contract is deployed on the network operator will deploy the Proxy contract. Parameters of the constructor: address of deployed target contract in the network and its initialization parameters.

When all needed Proxy contracts are deployed, one of them (which must implements `UpgradeableMaster` interface) will act as a parameter of constructor of `UpgradeGatekeeper` (it will names "`mainContract`").

The last part of deploying --- is to transfer mastership of all proxy contracts to the gatekeeper and add them to the gatekeeper's list of managing contracts.

The last will be done by calling several times the next function:

```sol
/// @notice Adds a new upgradeable contract to the list of contracts managed by the gatekeeper
/// @param addr Address of upgradeable contract to add
function addUpgradeable(address addr) external
```

### Upgrade process

There is three **phases of upgrade** which described in UpgradeGatekeeper:

```sol
/// @notice Upgrade mode statuses
enum UpgradeStatus {
    Idle,
    NoticePeriod,
    Preparation
}
```

* **Idle**
This is a phase when there are no upgrades to process.

* **NoticePeriod**
This phase starts when master of gatekeeper calls next function:
```sol
// @notice Starts upgrade (activates notice period)
/// @param newTargets New managed contracts targets (if element of this array is equal to zero address it means that appropriate upgradeable contract wouldn't be upgraded this time)
function startUpgrade(address[] calldata newTargets) external
```
Sense of this phase - give all users of the rollup contract an opportunity to withdraw funds to the ethereum network before updating the target. The transition to the next phase can be done after at least upgradeNoticePeriod seconds from the start of this phase. upgradeNoticePeriod is a value which defines from some "`mainContract`":
```sol
/// @notice Contract which defines notice period duration and allows finish upgrade during preparation of it
UpgradeableMaster public mainContract;
```

* **Preparation**
This is a finish phase. During this phase master can call finishUpgrade function.
```sol
/// @notice Finishes upgrade
/// @param targetsInitializationParameters New targets initialization parameters per each upgradeable contract
function finishUpgrade(bytes[] calldata targetsInitializationParameters) external {
```

One of the most important checks inside this function:
```sol
require(mainContract.readyForUpgrade(), "fpu13"); // fpu13 - main contract is not ready for upgrade
```

## Current Franklin contract upgrade specification or "How notice period works"

`UPGRADE_NOTICE_PERIOD` defined in Config.sol:

```sol
/// @notice Notice period before activation preparation status of upgrade mode (in seconds)
uint constant UPGRADE_NOTICE_PERIOD = 2 weeks;
```

`Franklin`'s decision about preparing for finishing the upgrade:
```sol
// @notice Checks that contract is ready for upgrade
/// @return bool flag indicating that contract is ready for upgrade
function readyForUpgrade() external returns (bool) {
    return !exodusMode && totalOpenPriorityRequests == 0;
}
```

To prevent rollup from spamming of priority requests, during `preparation lock period` contract will not add new priority requests.
This period starts from the notification from the gatekeeper about the start of preparation status of upgrade and ends when the upgrade finishes, or after `UPGRADE_PREPARATION_LOCK_PERIOD` seconds from its start.

The last is defined in Config.sol:
```sol
/// @notice Period after the start of preparation upgrade when contract wouldn't register new priority operations (in seconds)
uint constant UPGRADE_PREPARATION_LOCK_PERIOD = 1 days;
```