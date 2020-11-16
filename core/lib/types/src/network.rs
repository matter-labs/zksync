//! The network where the zkSync resides.
//!

// Built-in uses
use std::{fmt, str::FromStr};

// External uses
use serde::{Deserialize, Serialize};

// Workspace uses

// Local uses

/// Network to be used for a zkSync client.
///
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Network {
    /// Ethereum Mainnet.
    Mainnet,
    /// Ethereum Rinkeby testnet.
    Rinkeby,
    /// Ethereum Ropsten testnet.
    Ropsten,
    /// Self-hosted Ethereum & zkSync networks.
    Localhost,
    /// Unknown network type.
    Unknown,
}

impl FromStr for Network {
    type Err = String;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        Ok(match string {
            "mainnet" => Self::Mainnet,
            "rinkeby" => Self::Rinkeby,
            "ropsten" => Self::Ropsten,
            "localhost" => Self::Localhost,
            another => return Err(another.to_owned()),
        })
    }
}

impl fmt::Display for Network {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mainnet => write!(f, "mainnet"),
            Self::Rinkeby => write!(f, "rinkeby"),
            Self::Ropsten => write!(f, "ropsten"),
            Self::Localhost => write!(f, "localhost"),
            Self::Unknown => write!(f, "unknown"),
        }
    }
}

impl Network {
    /// Returns the network chain ID on the Ethereum side.
    pub fn chain_id(self) -> u8 {
        match self {
            Network::Mainnet => 1,
            Network::Ropsten => 3,
            Network::Rinkeby => 4,
            Network::Localhost => 9,
            Network::Unknown => panic!("Unknown chain ID"),
        }
    }
}
