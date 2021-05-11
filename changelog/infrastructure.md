# Infrastructure Changelog

All notable changes to the infrastructure will be documented in this file. Since the `infrastructure` has a lot of
components, the logs will have the following format:

```
(<component-name>): <the description of the change>
```

## Unreleased

### Removed

### Changed

### Added

- (`token_list_manager`): CLI for updating to new version of a previously saved list of trusted tokens.

- (`loadnext`): Crate, a new implementation of the loadtest for zkSync.

### Fixed

## Release 2021-02-19

### Removed

- (`ci/Dockerfile`): `docker/ci` folder was removed, because it is outdated.

### Changed

- (`fee-seller`): migrating to zksync V0.9.0.

### Added

- (`read-variable`): tool for read private and public variables from contracts.
- (`reading-tool`): tool for reading test config.
- (`explorer`): column "Can be used to pay fees" for tokens.

### Fixed

- (`fee-seller`): Sends all Ethereum transactions with sequential nonce starting with the next available not finalized
  nonce. Thereby resend stuck transactions after the next time you run the script.

- (`explorer`): Bug with 'Click to copy' button for account address.

## Release 2021-02-02

### Removed

- (`explorer`): 'localStorage' caching. It fixed the bug when the block have not updated the "Initiated" status.

### Changed

- (`explorer`): Deposits from and withdrawals to an L1 account are only displayed in the history of operation initiator.

### Added

- (`explorer`): `completeWithdrawals` tx hash was added to the explorer.

### Fixed

- (`explorer`): Bug with not displaying old blocks was fixed.
- (`explorer`): Bug with updating transaction data after searching another transaction was fixed.
- (`explorer`): Fixed processing of transactions with different prefixes.
- (`explorer`): bug with not displaying some deposits and withdrawals for the accounts was fixed by not taking account
  address case into account.

## Release 2021-01-12

### Added

- (`fee-seller`): reserve fee accumulator address.

### Changed

- (`explorer`) was refactored and optimized.
- (`explorer`): optimized by caching.
- (`tok_cli`): was removed and all commands from it have been moved to `zk`.

### Fixed

- (`fee-seller`): the logic of amount to withdraw/transfer through ZkSync network.
- Link to status page was added to explorer.
- (`explorer`): account and token ids, verified and committed nonces.
- (`zk`): `lint` command.
