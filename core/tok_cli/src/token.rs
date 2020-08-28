use crate::get_matches_from_lines;
use crate::run_external_command;

// Built-in deps
use std::time::{SystemTime, UNIX_EPOCH};

// External uses
use anyhow::Result;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use web3::{
    api::Eth,
    contract::Options,
    futures::Future,
    transports::Http,
    types::{Address, H256},
};

// Local uses
use models::node::tokens;
use server::api_server::admin_server::encode_token;

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
            "deploy-erc20.sh",
            &["test", name, symbol, &decimals.to_string()],
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
        endpoint_addr: std::net::SocketAddr,
        secret_auth: &str,
    ) -> Result<tokens::Token> {
        let client = Client::new();

        let start = SystemTime::now();
        let seconds_active = 15 * 60; // 15 minute
        let active_to = start.duration_since(UNIX_EPOCH)?.as_secs() + seconds_active;

        let query_to_tokens = format!("http://{}/tokens", endpoint_addr.to_string());
        let query_to_count = format!("http://{}/count", endpoint_addr.to_string());

        let auth_token = encode_token(secret_auth, "Authorization", active_to as usize)?;

        let id = client
            .get(&query_to_count)
            .bearer_auth(&auth_token)
            .send()
            .await?;

        if id.status() != reqwest::StatusCode::OK {
            return Err(anyhow::anyhow!(
                "Get query for get 'id' responded with a non-OK response: {}",
                id.status()
            ));
        }

        let id = id.text().await?.parse::<u16>()?;

        let erc20 = tokens::Token::new(id, self.address, &self.symbol, self.decimals);

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

        Ok(erc20)
    }
}
