// Built-in
// External
use jsonrpc_core::types::response::Output;
use serde::Serialize;
// use web3::types::{H160, H256};
// Workspace
// use jsonrpc_core::IoHandler;
// use jsonrpc_core_client::transports::http::connect;
use models::node::tx::TxHash;
use models::node::Address;
// use server::api_server::rpc_server::gen_client::Client as RpcClient;
use server::api_server::rpc_server::AccountInfoResp;
// use std::collections::VecDeque;

#[derive(Serialize)]
struct AccountStateReq {
    id: u32,
    method: String,
    jsonrpc: String,
    params: Vec<Address>,
}

impl AccountStateReq {
    fn new(address: Address) -> Self {
        Self {
            id: 1,
            method: "account_info".to_owned(),
            jsonrpc: "2.0".to_owned(),
            params: vec![address],
        }
    }
}

// requests and returns a tuple (executed, verified) for operation with given serial_id
pub async fn account_state_info(
    address: Address,
    rpc_addr: &str,
) -> Result<AccountInfoResp, failure::Error> {
    let msg = AccountStateReq::new(address);

    let client = reqwest::Client::new();
    let res = client.post(rpc_addr).json(&msg).send().await?;
    if res.status() != reqwest::StatusCode::OK {
        failure::bail!("non-ok response: {}", res.status());
    }
    let reply: Output = res.json().await.unwrap();
    let ret = match reply {
        Output::Success(v) => v.result,
        Output::Failure(v) => failure::bail!("rpc error: {}", v.error),
    };
    let account_state =
        serde_json::from_value(ret).expect("failed to parse account reqest responce");
    Ok(account_state)
}

#[derive(Serialize)]
struct EthopInfo {
    id: String,
    method: String,
    jsonrpc: String,
    params: Vec<u64>,
}
impl EthopInfo {
    fn new(serial_id: u64) -> Self {
        Self {
            id: "3".to_owned(),
            method: "ethop_info".to_owned(),
            jsonrpc: "2.0".to_owned(),
            params: vec![serial_id],
        }
    }
}

// requests and returns a tuple (executed, verified) for operation with given serial_id
pub async fn ethop_info(serial_id: u64, rpc_addr: &str) -> Result<(bool, bool), failure::Error> {
    let msg = EthopInfo::new(serial_id);

    let client = reqwest::Client::new();
    let res = client.post(rpc_addr).json(&msg).send().await?;
    if res.status() != reqwest::StatusCode::OK {
        failure::bail!("non-ok response: {}", res.status());
    }
    let reply: Output = res.json().await.unwrap();
    let ret = match reply {
        Output::Success(v) => v.result,
        Output::Failure(v) => panic!("{}", v.error),
    };
    let obj = ret.as_object().unwrap();
    let executed = obj["executed"].as_bool().unwrap();
    if !executed {
        return Ok((false, false));
    }
    let block = obj["block"].as_object().unwrap();
    let verified = block["verified"].as_bool().unwrap();
    Ok((executed, verified))
}

#[derive(Serialize)]
struct TxInfo {
    id: String,
    method: String,
    jsonrpc: String,
    params: Vec<TxHash>,
}

impl TxInfo {
    fn new(h: TxHash) -> Self {
        Self {
            id: "4".to_owned(),
            method: "tx_info".to_owned(),
            jsonrpc: "2.0".to_owned(),
            params: vec![h],
        }
    }
}

// requests and returns whether transaction is verified or not.
pub async fn tx_info(tx_hash: TxHash, rpc_addr: &str) -> Result<bool, failure::Error> {
    let msg = TxInfo::new(tx_hash);

    let client = reqwest::Client::new();
    let res = client.post(rpc_addr).json(&msg).send().await?;
    if res.status() != reqwest::StatusCode::OK {
        failure::bail!("non-ok response: {}", res.status());
    }
    let reply: Output = res.json().await.unwrap();
    let ret = match reply {
        Output::Success(v) => v.result,
        Output::Failure(v) => panic!("{}", v.error),
    };
    let obj = ret.as_object().unwrap();
    let executed = obj["executed"].as_bool().unwrap();
    if !executed {
        return Ok(false);
    }
    let block = obj["block"].as_object().unwrap();
    let verified = block["verified"].as_bool().unwrap();
    Ok(verified)
}
