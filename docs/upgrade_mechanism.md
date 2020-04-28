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

### Proxy

`Proxy` is an interim contract between the caller and the real implementation of the contract.

`contract Proxy is Upgradeable, UpgradeableMaster, Ownable`.

**Note: storage of this contract will be a context in which all processes of the target will work.** Proxy will store address of its **target** at the storage slot with index `keccak256("target")`.

As Proxy implements Upgradeable, it can change its target, but only `master` can do it.

Proxy have a fallback function that performs a delegatecall to the contract implementation and return whatever the implementation call returns

There is some type of calls that Proxy must intercept without uncheck submitting to processing to a fallback function: calling the `initialize` function (in the right way this function will be called from proxy contract directly) and functions of `UpgradeableMaster` interface (that is a reason why Proxy implements it).

### UpgradeGatekeeper

`UpgradeGatekeeper` is a contract that will manage the upgrade process. It is needed to prevent upgrading rollup contracts by the master in one function call.

## Deploying and upgrade process

### Deploying

When a target contract is deployed on the network operator will deploy the Proxy contract. Parameters of the constructor: address of deployed target contract in the network and its initialization parameters.

When all needed Proxy contracts are deployed, one of them (which must implements `UpgradeableMaster` interface) will act as a parameter of constructor of `UpgradeGatekeeper` (it will names "`mainContract`").

The last part of deploying --- is to transfer mastership of all proxy contracts to the gatekeeper and add them to the gatekeeper's list of managing contracts.

The last should be done by calling several times `addUpgradeable` function.

### Upgrade process

There is three **phases of upgrade** which described in UpgradeGatekeeper:

* **Idle**
This is a phase when there are no upgrades to process.

* **NoticePeriod**
This phase starts when master of gatekeeper calls `startUpgrade` function.
Sense of this phase --- give all users of the rollup contract an opportunity to withdraw funds to the ethereum network before updating the target. The transition to the next phase can be done after at least upgradeNoticePeriod seconds from the start of this phase. `upgradeNoticePeriod` is a value which defines by the "`mainContract`".

* **Preparation**
This is a finish phase. During this phase master can call `finishUpgrade` function.
One of the most important checks inside this function is enforcing that the mainContract is ready for the upgrade.

## Current Franklin contract upgrade specification or "How notice period works"

`UPGRADE_NOTICE_PERIOD` defined in Config.sol as 2 weeks.

`Franklin` makes a decision that upgrade can be finished when both of these conditions is true:
1. exodus mode is not activated
2. count of open priority requests equals to zero.

To prevent rollup from spamming of priority requests, during `preparation lock period` contract will not add new priority requests.
This period starts from the notification from the gatekeeper about the start of preparation status of upgrade and ends when the upgrade finishes, or after `UPGRADE_PREPARATION_LOCK_PERIOD` seconds from its start.

`UPGRADE_PREPARATION_LOCK_PERIOD` is defined in Config.sol as 1 day.