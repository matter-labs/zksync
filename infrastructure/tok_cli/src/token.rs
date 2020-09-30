use crate::get_matches_from_lines;
use crate::run_external_command;
use crate::utils::encode_auth_token;

// Built-in deps
use std::time::{SystemTime, UNIX_EPOCH};

// External uses
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use url::Url;
use web3::{
    api::Eth,
    contract::Options,
    futures::Future,
    transports::Http,
    types::{Address, H256},
};

// Workspace uses
use zksync_types::{tokens, TokenId};

/// Token that contains information to add to the server
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct AddTokenRequest {
    /// id is used for tx signature and serialization
    /// is optional because when adding the server will assign the next available ID
    pub id: Option<TokenId>,
    /// Contract address of ERC20 token or Address::zero() for "ETH"
    pub address: Address,
    /// Token symbol (e.g. "ETH" or "USDC")
    pub symbol: String,
    /// Token precision (e.g. 18 for "ETH" so "1.0" ETH = 10e18 as U256 number)
    pub decimals: u8,
}

impl AddTokenRequest {
    pub fn new(id: Option<TokenId>, address: Address, symbol: &str, decimals: u8) -> Self {
        Self {
            id,
            address,
            symbol: symbol.to_string(),
            decimals,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Token {
    pub address: Address,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

impl Token {
    pub fn new(address: Address, name: &str, symbol: &str, decimals: u8) -> Self {
        Self {
            address,
            name: name.to_string(),
            symbol: symbol.to_string(),
            decimals,
        }
    }

    pub async fn get_info_about_token(address: Address, eth: Eth<Http>) -> Result<Self> {
        let contract =
            web3::contract::Contract::from_json(eth, address, include_bytes!("./../erc20.json"))
                .map_err(|_e| anyhow::anyhow!("Error create contract ABI for ERC20"))?;

        let name: String = contract
            .query("name", (), None, Options::default(), None)
            .wait()
            .map_err(|_e| anyhow::anyhow!("Token does not support the 'name' field"))?;

        let decimals: u8 = contract
            .query("decimals", (), None, Options::default(), None)
            .wait()
            .map_err(|_e| anyhow::anyhow!("Token does not support the 'decimals' field"))?;

        let symbol: String = contract
            .query("symbol", (), None, Options::default(), None)
            .wait()
            .map_err(|_e| anyhow::anyhow!("Token does not support the 'symbol' field"))?;

        Ok(Self::new(address, &name, &symbol, decimals))
    }

    pub async fn deploy_test_token(name: &str, decimals: u8, symbol: &str) -> Result<Token> {
        let stdout = run_external_command(
            "deploy-erc20",
            &["new", name, symbol, &decimals.to_string()],
        )?;

        serde_json::from_str(&stdout).map_err(|_e| anyhow::anyhow!("Error decode token from json"))
    }

    pub async fn add_to_governance(address: Address, key: H256) -> Result<()> {
        let stdout = run_external_command(
            "governance-add-erc20.sh",
            &[&address.to_string()["0x".len()..], &key.to_string()],
        )?;

        let line = get_matches_from_lines(&stdout, "STATUS=")?;
        if &line["STATUS=".len()..] == "SUCCESS" {
            Ok(())
        } else {
            Err(anyhow::anyhow!("Error add token to governance"))
        }
    }

    pub async fn add_to_server(
        &self,
        endpoint_addr: Url,
        secret_auth: &str,
    ) -> Result<tokens::Token> {
        let client = Client::new();

        let start = SystemTime::now();
        let seconds_active = 15 * 60; // 15 minutes
        let active_to = start.duration_since(UNIX_EPOCH)?.as_secs() + seconds_active;

        let query_to_tokens = format!("{}tokens", endpoint_addr.to_string());

        let auth_token = encode_auth_token(secret_auth, "Authorization", active_to as usize)?;

        let erc20 = AddTokenRequest::new(None, self.address, &self.symbol, self.decimals);

        let res = client
            .post(&query_to_tokens)
            .bearer_auth(&auth_token)
            .json(&erc20)
            .send()
            .await?;

        if res.status() != reqwest::StatusCode::OK {
            return Err(anyhow::anyhow!(
                "Post query to tokens responded with a non-OK response: {}",
                res.status()
            ));
        }

        Ok(res.json::<tokens::Token>().await?)
    }
}
