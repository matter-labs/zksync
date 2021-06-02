# Smart Contracts' Changelog

All notable changes to the contracts will be documented in this file.

## 2021-02-06

**Version 5.1** is scheduled for upgrade.

### Added

- `MintNFT`, `WithdrawNFT` operations, which enable native NFT support.
- `Swap` operation, which depending on the implementation may serve either order book or atomic swaps functionality.
- The security council, which is able to shorten the upgrade notice period. More on that
  [here](https://medium.com/matter-labs/keeping-funds-safe-a-3-factor-approach-to-security-in-zksync-2-0-a70b0f53f360).
- `RegeneisMultisig.sol` to handle submissions of the new root hash by the security council during the upgrade.
- A special account with id `2**24 - 1`. It is used to ensure the correctness of the NFT data.

### Changed

- The maximum amount of tokens that can be used to pay transaction fees is increased to 1023.
- The account tree depth for each account increased to `32`.
- Due to the smart contract size limitations, `ZkSync.sol` was split into two parts: the original `ZkSync.sol` and the
  `AdditionalZkSync.sol`, to which some functionality is delegated.
- `ZkSyncNFTFactory.sol` which serves as a default NFT factory, where all the NFTs are withdrawn by default.

## 2021-25-02

**Version 5** is scheduled for upgrade.

### Added

- `tokenGovernance` address is added to the `Governance` contract. `tokenGovernance` can list new tokens.
- `TokenGovernance` contract is added to allow anybody to pay fee and list new tokens.

### Changed

- Maximum amount of tokens that can be used to pay tx fee is increased to 512.
- Circuit now enforces that `ForcedExit` target account pubkey hash is empty.

## 2021-09-02

**Version 4** is released.

## 2021-14-01

**Version 4** is scheduled for upgrade.

### Added

- Multiple blocks can be committed, proven or executed with one transaction.
- Rollup block timestamp is added.
- `ChangePubKey` can be authorized without L1 transaction for smart-contract accounts that can be deployed using CREATE2
  function with specific salt.
- Block processing is split into three parts: commit, proving onchain, execution. Block is finalized when its executed.
- Governance now can pause deposits of some tokens.
- New events `Deposit(token, amount)`, `Withdrawal(token, amount)` are added when funds are deposited or removed from
  the contract.

### Changed

- Cost of `Deposit` and `FullExit` for user is reduced significantly.
- Priority queue storage format is changed for gas cost optimization.
- Block storage format is changed for gas cost optimization.
- `ChangePubKey` message that should be signed by ETH private key is changed.
- Upgrade notice period is increased to 2 weeks.
- `ChangePubKey` with L2 public key that was authorized onchain can be reset if needed after waiting period of 1 day.
- Maximum gas limit for ETH and ERC20 token withdrawal from zkSync contract is set to be 100k.
- Proof of the L2 funds in the exodus mode now can be provided by anyone on behalf of any user.
- Withdrawal from zkSync contract (funds that are withdrawn from L2 but failed to be pushed out of the zkSync contract)
  now can be done on behalf of any user.
- Some of the public variables are made internal.
- Withdraw from the zkSync contract now should be performed using `withdrawPendingBalance` instead of (`withdrawETH`,
  `withdrawERC20`).
- Multiple functions and variables are renamed:
  1. `fullExit` -> `requestFullExit`
  1. `exit` -> `performExodus`
  1. `triggerExodusIfNeeded` -> `activateExodusMode`
  1. `balanceToWithdraw` -> `pendingBalance`

### Removed

- Pending withdrawal queue is removed, instead we try to execute token transfers when we finalize block.
- All events for user operations (Deposit, Withdraw, etc.) are removed.

## 2020-09-04

**Version 3** is released.

### Added

- `ForcedExit` operation which allows user to force a withdrawal from another account that does not have signing key set
  and is older than 24h.

### Changed

- `ChangePubKey` operation requires fee for processing.

## 2020-07-20

**Version 2** is released.

### Added

- Event denoting information about pending and completed withdrawals.
- Support for tokens that aren't fully compatible with ERC20.

### Changed

- Block revert interval is 0 hours.
- `PRIORITY_EXPRIRATION_PERIOD` is reduced to 3 days.
- `UPGRADE_NOTICE_PERIOD` is increased to 8 days.

### Removed

- Redundant priority request check is removed from contract upgrade logic.
