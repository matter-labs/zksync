// Built-in uses
use std::collections::HashMap;
use std::convert::TryInto;
use std::str::FromStr;
// External uses
use ethabi::{encode, Contract, Function, Param, ParamType, Token as AbiToken};
use jsonrpc_core::{Error, Result};
use tiny_keccak::keccak256;
// Workspace uses
use zksync_storage::StorageProcessor;
use zksync_types::{Token, TokenId, NFT};
// Local uses
use super::{
    converter::u256_from_biguint,
    types::{Bytes, H160, H256, U256},
    ZKSYNC_PROXY_ADDRESS,
};
use crate::api_server::web3::types::CallRequest;
use crate::utils::token_db_cache::TokenDBCache;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CallsHelper {
    erc20: HashMap<[u8; 4], Function>,
    zksync_proxy: HashMap<[u8; 4], Function>,
    tokens: TokenDBCache,
    zksync_proxy_address: H160,
}

impl CallsHelper {
    fn gen_hashmap(functions: Vec<Function>) -> HashMap<[u8; 4], Function> {
        functions
            .into_iter()
            .map(|f| {
                let inputs = f
                    .inputs
                    .iter()
                    .map(|p| p.kind.to_string())
                    .collect::<Vec<_>>()
                    .join(",");
                let signature = format!("{}({})", f.name, inputs);
                let selector: [u8; 4] = keccak256(signature.as_bytes())[0..4].try_into().unwrap();
                (selector, f)
            })
            .collect()
    }

    pub fn new() -> Self {
        let mut path = PathBuf::new();
        path.push(std::env::var("ZKSYNC_HOME").unwrap_or_else(|_| "/".to_string()));
        path.push("core/bin/zksync_api/src/api_server/web3/abi");
        let erc20_abi = std::fs::File::open(path.join("ERC20.json")).unwrap();
        let erc20_functions = Contract::load(erc20_abi)
            .unwrap()
            .functions
            .values()
            .flatten()
            .cloned()
            .collect();
        let erc20_function_by_selector = Self::gen_hashmap(erc20_functions);

        let zksync_proxy_abi = std::fs::File::open(path.join("ZkSyncProxy.json")).unwrap();
        let zksync_proxy_functions = Contract::load(zksync_proxy_abi)
            .unwrap()
            .functions
            .values()
            .flatten()
            .cloned()
            .collect();
        let zksync_proxy_function_by_selector = Self::gen_hashmap(zksync_proxy_functions);

        Self {
            erc20: erc20_function_by_selector,
            zksync_proxy: zksync_proxy_function_by_selector,
            tokens: TokenDBCache::new(),
            zksync_proxy_address: H160::from_str(ZKSYNC_PROXY_ADDRESS).unwrap(),
        }
    }

    pub fn execute(
        &self,
        storage: &mut StorageProcessor<'_>,
        to: H160,
        data: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let all_functions = if to == self.zksync_proxy_address {
            &self.zksync_proxy
        } else {
            let token = self
                .tokens
                .get_token(storage, to)
                .await
                .map_err(|_| Error::internal_error())?;
            if let Some(token) = token {
                if !token.is_nft {
                    &self.erc20
                } else {
                    return Ok(Vec::new());
                }
            } else {
                return Ok(Vec::new());
            }
        };
        let selector: [u8; 4] = if data.len() >= 4 {
            data[0..4].try_into().unwrap()
        } else {
            return Ok(Vec::new());
        };
        let function = all_functions.get(&selector)?;
        let params = if let Ok(params) = function.decode_input(&data[4..]) {
            params
        } else {
            return Ok(Vec::new());
        };

        let result = if to == self.zksync_proxy_address {
            Vec::new()
        } else {
            let token = self
                .tokens
                .get_token(storage, to)
                .await
                .map_err(|_| Error::internal_error())?
                .ok_or_else(Error::internal_error)?;
            match function.name.as_str() {
                "name" | "symbol" => encode(&[AbiToken::String(token.symbol)]),
                "decimals" => encode(&[AbiToken::Uint(U256::from(token.decimals))]),
                "totalSupply" | "allowance" => encode(&[AbiToken::Uint(U256::max_value())]),
                "balanceOf" => {
                    let block = storage
                        .chain()
                        .block_schema()
                        .get_last_saved_block()
                        .await
                        .map_err(|_| Error::internal_error())?;
                    let address = params[0]
                        .into_address()
                        .ok_or_else(Error::internal_error())?;
                    let balance = storage
                        .chain()
                        .account_schema()
                        .get_account_balance_for_block(address, block, token.id)
                        .await
                        .map_err(|_| Error::internal_error())?;
                    encode(&[AbiToken::Uint(u256_from_biguint(balance)?)])
                }
                _ => unreachable!(),
            }
        };
        Ok(result)
    }
}
