//! Real-life loadtest scenario does not measure the TPS nor simulated the high load,
//! but rather simulates the real-life use case of zkSync:
//!
//! 1. Funds are deposited from one Ethereum account into one new zkSync account.
//! 2. Once funds are deposited, this account split the funds between N accounts
//!    using the `transferToNew` operation.
//! 3. Once funds are transferred and verified, these funds are "rotated" within
//!    created accounts using the `transfer` operation. This operation is repeated
//!    M times.
//! 4. To finish the test, all the funds are collected back to the initial account
//!    are withdrawn to the Ethereum.
//!
//! `N` and `M` are configurable parameters, meaning the breadth of the test (how
//! many accounts will be used within the test) and the depth of the test (how
//! many rotation cycles are performed) correspondingly.
//!
//! Schematically, scenario will look like this:
//!
//! Deposit  | Transfer to new  | Transfer | Collect back | Withdraw to ETH
//!
//! ```text
//!                                ┗━━━━┓
//!                      ┏━━━>Acc1━━━━━┓┗>Acc1━━━┓
//!                    ┏━┻━━━>Acc2━━━━┓┗━>Acc2━━━┻┓
//! ETH━━━━>InitialAcc━╋━━━━━>Acc3━━━┓┗━━>Acc3━━━━╋━>InitialAcc━>ETH
//!                    ┗━┳━━━>Acc4━━┓┗━━━>Acc4━━━┳┛
//!                      ┗━━━>Acc5━┓┗━━━━>Acc5━━━┛
//! ```
