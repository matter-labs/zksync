# Upgrade mechanism

This is an image of the dependencies structure.
Use it for a better understanding.
![](https://docs.google.com/drawings/d/e/2PACX-1vQWlvxseJXa-X8PhrkpshBiE_rlJJak4noE2wl__0uH957MHK2jLlzxWMfOMsr7AnzpfMqga52bn-Oc/pub?w=960&h=720)

## Contracts and interfaces

### Ownable

`Ownable` is a contract that stores the address of its **master** at the storage slot with index `keccak256("master")`.

### UpgradeableMaster

`UpgradeableMaster` is an interface that controls the upgrade flow through the **phases of upgrade** (this will be explained below)

### Upgradeable

`Upgradeable` is an interface of a proxy with a variable **target** (address of implementation). The target can be changed by calling `upgradeTarget` method on the proxy.

### Proxy

`Proxy` is an interim contract between the caller and the real implementation of the contract.

`contract Proxy is Upgradeable, UpgradeableMaster, Ownable`.

**Note: storage of this contract will be a context in which all processes of the target will work.** Proxy will store address of its **target** at the storage slot with index `keccak256("target")`.

`Proxy` implements `Upgradeable` interface: `master` of Proxy can change its target.

Proxy has a fallback function that performs a delegatecall to the contract implementation and returns whatever the implementation call returns.

There is some type of calls that Proxy must intercept without uncheck submitting to processing to a fallback function: calling the `initialize` function (in the right way this function will be called from proxy contract directly) and functions of `UpgradeableMaster` interface (that is a reason why Proxy implements it).

### UpgradeGatekeeper

`UpgradeGatekeeper` is a contract that will manage the upgrade process. It is needed to prevent upgrading rollup contracts by the master in one function call and do upgrades of all contracts atomically: contracts that are declared to be upgraded at the start of this process should all be upgraded, or all these upgrades will be rejected.

## Deploying and upgrade process

### Deploying

After the target contract is deployed, operator will deploy the Proxy contract. This will initialize the storage.

When all needed Proxy contracts are deployed, one of them (which must implements `UpgradeableMaster` interface) will act as a parameter of constructor of `UpgradeGatekeeper` (it will names "`mainContract`").

The last part of the deploy process is to transfer all proxy contracts under control of the gatekeeper. This should be done by calling several times `addUpgradeable` function.

---

**So, step-by-step deploy process:**

1. Deploy targets for all proxies.
2. Deploy proxies with needed target addresses and initialization parameters.
3. Deploy UpgradeGatekeeper passing the address of the main proxy as a parameter.
4. Transfer mastership of all proxies to the `UpgradeGatekeeper`.
5. Add all proxies to the gatekeeper's list by calling `UpgradeGatekeeper.addUpgradeable()` function for each of them.

---

### Upgrade process

There is three **phases of upgrade** which described in UpgradeGatekeeper:

|Phase|Description|
|-|-|
|**Idle**|This is a phase when there are no upgrades to process.|
|**NoticePeriod**|This phase starts when master of gatekeeper calls `startUpgrade` function. The purpose of this phase is to give all users of the rollup contract an opportunity to withdraw funds to the ethereum network before upgrading the target. The transition to the next phase can be done after at least upgradeNoticePeriod seconds from the start of this phase. `upgradeNoticePeriod` is a value which defines by the "`mainContract`".|
|**Preparation**|This is the final phase. During this phase, master can call `finishUpgrade` function, which upgrades targets in proxies. One of the most important checks inside this function is enforcing that the mainContract is ready for the upgrade.|

---

**So, step-by-step upgrade process:**

1. Deploy new targets for proxies that would be upgraded.
2. Call `startUpgrade` with addresses of these targets (and zeroes for proxies that shouldn't be upgraded).
3. Wait for the end of the `notice period`.
4. Call `startPreparation`.
5. Wait until all open priority operations will be processed.
6. Call `finishUpgrade` with needed targets initialization parameters.

**NOTE:** In case of some error or bug in code, there is an option to cancel the upgrade before finishing by calling `cancelUpgrade` function.

---

## Current Franklin contract upgrade specification or "How notice period works"

`UPGRADE_NOTICE_PERIOD` defined in Config.sol as 1 week.

`Franklin` decides that the upgrade can be completed if both of these conditions are met:
1. exodus mode is not activated
2. number of open priority requests equals to zero.

To prevent rollup from spamming of priority requests, during `preparation lock period` contract will not add new priority requests.
This period starts from the notification from the gatekeeper about the start of preparation status of upgrade and ends when the upgrade finishes, or after `UPGRADE_PREPARATION_LOCK_PERIOD` seconds from its start.

`UPGRADE_PREPARATION_LOCK_PERIOD` is defined in Config.sol as 1 day.