//! Satellite scenario for real-life loadtest.
//!
//! Satellite scenario is ran concurrently to the main scenario
//! and it performs several deposit / withdraw operations at the same
//! time as the funds are rotated in the main scenario.
//!
//! The purpose of the satellite scenario is to ensure that deposits
//! and withdraws are processed correctly when the node is under a
//! load of many transfers.
