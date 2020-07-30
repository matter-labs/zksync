# Upgrade mechanism

This is an image of the dependencies structure.
Use it for a better understanding.
![](https://docs.google.com/drawings/d/e/2PACX-1vQWlvxseJXa-X8PhrkpshBiE_rlJJak4noE2wl__0uH957MHK2jLlzxWMfOMsr7AnzpfMqga52bn-Oc/pub?w=960&h=720)
Source: [link](https://docs.google.com/drawings/d/13SlGac7BHqFeL0J0J3BHdn_nx2tZcbN_u4kWn8Q7t7c/edit)

## Contracts and interfaces

### Ownable

`Ownable` is a contract that stores the address of its **master** at the storage slot with index `bytes32(uint256(keccak256('eip1967.proxy.admin')) - 1)`.

### UpgradeableMaster

`UpgradeableMaster` is an interface that controls the upgrade flow through the **phases of upgrade** (this will be explained below)

### Upgradeable

`Upgradeable` is an interface of a proxy with a variable **target** (address of implementation). The target can be changed by calling `upgradeTarget` method on the proxy.

### Proxy

`Proxy` is an interim contract between the caller and the real implementation of the contract.

`contract Proxy is Upgradeable, UpgradeableMaster, Ownable`.

**Note: storage of this contract will be a context in which all processes of the target will work.** Proxy will store address of its **target** at the storage slot with index `bytes32(uint256(keccak256('eip1967.proxy.implementation')) - 1)`.

`Proxy` implements `Upgradeable` interface: `master` of Proxy can change its target.

Proxy has a fallback function that performs a delegatecall to the contract implementation and returns whatever the implementation call returns.

There is some type of calls that Proxy must intercept without unchecked submitting to processing to a fallback function. These are calls to `initialize` function (in the right way this function should be called only from `upgradeTarget` method and proxy's constructor) and calls to functions from `UpgradeableMaster` interface (that is why Proxy implements it).

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

`UPGRADE_NOTICE_PERIOD` defined in Config.sol.

`Franklin` decides that the upgrade can be completed if the next condition satisfied: exodus mode is not activated.

`UPGRADE_NOTICE_PERIOD` calculates by the next **formula**:
`UPGRADE_NOTICE_PERIOD = MASS_FULL_EXIT_PERIOD + PRIORITY_EXPIRATION_PERIOD + TIME_TO_WITHDRAW_FUNDS_FROM_FULL_EXIT`.
