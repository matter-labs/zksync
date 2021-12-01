//! API server handles endpoints for interaction with node.
//!
//! `mod rest` - api is used for block explorer.
//! `mod rpc_server` - JSON rpc via HTTP (for request reply functions)
//! `mod rpc_subscriptions` - JSON rpc via WebSocket (for request reply functions and subscriptions)

// External uses
use futures::channel::mpsc;
// Workspace uses

use zksync_eth_client::EthereumGateway;
use zksync_storage::ConnectionPool;
// Local uses
use crate::fee_ticker::TickerRequest;

mod event_notify;
pub mod forced_exit_checker;
mod helpers;
pub mod rest;
pub mod rpc_server;
pub mod rpc_subscriptions;
mod tx_sender;
pub mod web3;

/// Amount of threads used by each server to serve requests.
const THREADS_PER_SERVER: usize = 128;
