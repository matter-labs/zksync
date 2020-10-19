# zkSync types. Essential types for the zkSync network

`zksync_types` is a crate containing essential zkSync network types, such as transactions, operations and
blockchain primitives.

zkSync operations are split into the following categories:

- **transactions**: operations of zkSync network existing purely in the L2.
  Currently includes `Transfer`, `Withdraw`, `ChangePubKey` and `ForcedExit`.
  All the transactions form an enum named `ZkSyncTx`.
- **priority operations**: operations of zkSync network which are triggered by
  invoking the zkSync smart contract method in L1. These operations are discovered by
  the zkSync server and included into the block just like L2 transactions.
  Currently includes `Deposit` and `FullExit`.
  All the priority operations form an enum named `ZkSyncPriorityOp`.
- **operations**: a superset of `ZkSyncTx` and `ZkSyncPriorityOp`.
  All the operations are included into an enum named `ZkSyncOp`. This enum contains
  all the items that can be included into the block, together with meta-information
  about each transaction.
  Main difference of operation from transaction/priority operation is that it can form
  public data required for the committing the block on the L1.

## License

`zksync_models` is a part of zkSync stack, which is distributed under the terms of both the MIT license
and the Apache License (Version 2.0).

See [LICENSE-APACHE](../../LICENSE-APACHE), [LICENSE-MIT](../../LICENSE-MIT) for details.
