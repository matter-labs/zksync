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

- reading-tool for reading test config.

### Fixed

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
