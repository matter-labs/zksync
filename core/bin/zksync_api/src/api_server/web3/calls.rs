// Built-in uses
use std::collections::HashMap;
use std::convert::TryInto;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;
// External uses
use ethabi::{encode, Contract, Function, Token as AbiToken};
use jsonrpc_core::{Error, ErrorCode, Result};
use tiny_keccak::keccak256;
// Workspace uses
use zksync_storage::StorageProcessor;
use zksync_token_db_cache::TokenDBCache;
use zksync_types::{TokenId, TokenKind, NFT};

// Local uses
use super::{
    converter::u256_from_biguint,
    types::{H160, U256},
    NFT_FACTORY_ADDRESS, ZKSYNC_PROXY_ADDRESS,
};

type Selector = [u8; 4];

#[derive(Debug, Clone)]
pub struct CallsHelper {
    erc20: HashMap<Selector, Function>,
    nft_factory: HashMap<Selector, Function>,
    tokens: TokenDBCache,
    zksync_proxy_address: H160,
    nft_factory_address: H160,
}

impl CallsHelper {
    const SHA256_MULTI_HASH: [u8; 2] = [18, 32]; // 0x1220
    const ALPHABET: &'static str = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

    fn revert_error(message: &str) -> Error {
        Error {
            code: ErrorCode::ServerError(3),
            message: message.to_string(),
            data: None,
        }
    }

    fn function_by_selector(functions: Vec<Function>) -> HashMap<Selector, Function> {
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
                let selector: Selector = keccak256(signature.as_bytes())[0..4].try_into().unwrap();
                (selector, f)
            })
            .collect()
    }

    pub fn new() -> Self {
        let mut path = PathBuf::new();
        path.push(std::env::var("ZKSYNC_HOME").unwrap_or_else(|_| "/".to_string()));
        path.push("etc/web3-abi");
        let erc20_abi = std::fs::File::open(path.join("ERC20.json")).unwrap();
        let erc20_functions = Contract::load(erc20_abi)
            .unwrap()
            .functions
            .values()
            .flatten()
            .cloned()
            .collect();
        let erc20_function_by_selector = Self::function_by_selector(erc20_functions);

        let nft_factory_abi = std::fs::File::open(path.join("NFTFactory.json")).unwrap();
        let nft_factory_functions = Contract::load(nft_factory_abi)
            .unwrap()
            .functions
            .values()
            .flatten()
            .cloned()
            .collect();
        let nft_factory_function_by_selector = Self::function_by_selector(nft_factory_functions);

        Self {
            erc20: erc20_function_by_selector,
            nft_factory: nft_factory_function_by_selector,
            tokens: TokenDBCache::new(Duration::from_secs(5 * 60)),
            zksync_proxy_address: H160::from_str(ZKSYNC_PROXY_ADDRESS).unwrap(),
            nft_factory_address: H160::from_str(NFT_FACTORY_ADDRESS).unwrap(),
        }
    }

    pub async fn execute(
        &self,
        storage: &mut StorageProcessor<'_>,
        to: H160,
        data: Vec<u8>,
    ) -> Result<Vec<u8>> {
        let mut transaction = storage
            .start_transaction()
            .await
            .map_err(|_| Error::internal_error())?;
        let all_functions = if to == self.nft_factory_address {
            &self.nft_factory
        } else {
            let token = self
                .tokens
                .get_token(&mut transaction, to)
                .await
                .map_err(|_| Error::internal_error())?;
            match token {
                Some(token) if matches!(token.kind, TokenKind::ERC20) => &self.erc20,
                _ => return Ok(Vec::new()),
            }
        };
        let selector: Selector = if data.len() >= 4 {
            data[0..4].try_into().unwrap()
        } else {
            return Ok(Vec::new());
        };
        let function = if let Some(function) = all_functions.get(&selector) {
            function
        } else {
            return Ok(Vec::new());
        };
        let params = if let Ok(params) = function.decode_input(&data[4..]) {
            params
        } else {
            return Ok(Vec::new());
        };

        let result = if to == self.nft_factory_address {
            match function.name.as_str() {
                "creatorId" => {
                    let token_id = params[0]
                        .clone()
                        .into_uint()
                        .ok_or_else(Error::internal_error)?;
                    if let Some(nft) = self.get_nft(&mut transaction, token_id).await? {
                        encode(&[AbiToken::Uint(U256::from(nft.creator_id.0))])
                    } else {
                        return Err(Self::revert_error(
                            "execution reverted: creator ID query for nonexistent token",
                        ));
                    }
                }
                "creatorAddress" => {
                    let token_id = params[0]
                        .clone()
                        .into_uint()
                        .ok_or_else(Error::internal_error)?;
                    if let Some(nft) = self.get_nft(&mut transaction, token_id).await? {
                        encode(&[AbiToken::Address(nft.creator_address)])
                    } else {
                        return Err(Self::revert_error(
                            "execution reverted: creator address query for nonexistent token",
                        ));
                    }
                }
                "serialId" => {
                    let token_id = params[0]
                        .clone()
                        .into_uint()
                        .ok_or_else(Error::internal_error)?;
                    if let Some(nft) = self.get_nft(&mut transaction, token_id).await? {
                        encode(&[AbiToken::Uint(U256::from(nft.serial_id))])
                    } else {
                        return Err(Self::revert_error(
                            "execution reverted: serial ID query for nonexistent token",
                        ));
                    }
                }
                "contentHash" => {
                    let token_id = params[0]
                        .clone()
                        .into_uint()
                        .ok_or_else(Error::internal_error)?;
                    if let Some(nft) = self.get_nft(&mut transaction, token_id).await? {
                        encode(&[AbiToken::FixedBytes(nft.content_hash.as_bytes().to_vec())])
                    } else {
                        return Err(Self::revert_error(
                            "execution reverted: content hash query for nonexistent token",
                        ));
                    }
                }
                "tokenURI" => {
                    let token_id = params[0]
                        .clone()
                        .into_uint()
                        .ok_or_else(Error::internal_error)?;
                    if let Some(nft) = self.get_nft(&mut transaction, token_id).await? {
                        let ipfs_cid = Self::ipfs_cid(nft.content_hash.as_bytes());
                        encode(&[AbiToken::String(format!("ipfs://{}", ipfs_cid))])
                    } else {
                        return Err(Self::revert_error(
                            "execution reverted: ERC721Metadata: URI query for nonexistent token",
                        ));
                    }
                }
                "balanceOf" => {
                    let address = params[0]
                        .clone()
                        .into_address()
                        .ok_or_else(Error::internal_error)?;
                    if address.is_zero() {
                        return Err(Self::revert_error(
                            "execution reverted: ERC721: balance query for the zero address",
                        ));
                    }
                    let balance = transaction
                        .chain()
                        .account_schema()
                        .get_account_nft_balance(address)
                        .await
                        .map_err(|_| Error::internal_error())?;
                    encode(&[AbiToken::Uint(U256::from(balance))])
                }
                "ownerOf" => {
                    let token_id = params[0]
                        .clone()
                        .into_uint()
                        .ok_or_else(Error::internal_error)?;
                    if let Some(nft) = self.get_nft(&mut transaction, token_id).await? {
                        let owner_id = transaction
                            .chain()
                            .account_schema()
                            .get_nft_owner(nft.id)
                            .await
                            .map_err(|_| Error::internal_error())?;
                        let owner_address = if let Some(owner_id) = owner_id {
                            let owner_address = transaction
                                .chain()
                                .account_schema()
                                .account_address_by_id(owner_id)
                                .await
                                .map_err(|_| Error::internal_error())?;
                            owner_address.unwrap_or_default()
                        } else {
                            H160::zero()
                        };
                        encode(&[AbiToken::Address(owner_address)])
                    } else {
                        return Err(Self::revert_error(
                            "execution reverted: ERC721: owner query for nonexistent token",
                        ));
                    }
                }
                "getApproved" => {
                    let token_id = params[0]
                        .clone()
                        .into_uint()
                        .ok_or_else(Error::internal_error)?;
                    if self.get_nft(&mut transaction, token_id).await?.is_some() {
                        encode(&[AbiToken::Address(self.zksync_proxy_address)])
                    } else {
                        return Err(Self::revert_error(
                            "execution reverted: ERC721: approved query for nonexistent token",
                        ));
                    }
                }
                _ => unreachable!(),
            }
        } else {
            let token = self
                .tokens
                .get_token(&mut transaction, to)
                .await
                .map_err(|_| Error::internal_error())?
                .ok_or_else(Error::internal_error)?;
            match function.name.as_str() {
                "name" | "symbol" => encode(&[AbiToken::String(token.symbol)]),
                "decimals" => encode(&[AbiToken::Uint(U256::from(token.decimals))]),
                "totalSupply" | "allowance" => encode(&[AbiToken::Uint(U256::max_value())]),
                "balanceOf" => {
                    let block = transaction
                        .chain()
                        .block_schema()
                        .get_last_verified_confirmed_block()
                        .await
                        .map_err(|_| Error::internal_error())?;
                    let address = params[0]
                        .clone()
                        .into_address()
                        .ok_or_else(Error::internal_error)?;
                    let balance = transaction
                        .chain()
                        .account_schema()
                        .get_account_balance_for_block(address, block, token.id)
                        .await
                        .map_err(|_| Error::internal_error())?;
                    encode(&[AbiToken::Uint(u256_from_biguint(balance))])
                }
                _ => unreachable!(),
            }
        };
        transaction
            .commit()
            .await
            .map_err(|_| Error::internal_error())?;
        Ok(result)
    }

    async fn get_nft(
        &self,
        storage: &mut StorageProcessor<'_>,
        token_id: U256,
    ) -> Result<Option<NFT>> {
        if token_id > U256::from(u32::MAX) {
            return Ok(None);
        }
        let nft = self
            .tokens
            .get_nft_by_id(storage, TokenId(token_id.as_u32()))
            .await
            .map_err(|_| Error::internal_error())?;
        Ok(nft)
    }

    fn bytes_to_base58(source: &[u8]) -> String {
        let mut digits: [u8; 46] = [0; 46];
        let mut digit_length: usize = 1;
        for mut carry in source.iter().map(|a| *a as u32) {
            for digit in digits.iter_mut().take(digit_length) {
                carry += (*digit as u32) * 256;
                *digit = (carry % 58) as u8;
                carry /= 58;
            }

            while carry > 0 {
                digits[digit_length] = (carry % 58) as u8;
                digit_length += 1;
                carry /= 58;
            }
        }

        let result: Vec<u8> = digits.iter().rev().copied().collect();
        Self::indices_to_alphabet(&result)
    }

    pub fn ipfs_cid(source: &[u8]) -> String {
        let concat: Vec<u8> = Self::SHA256_MULTI_HASH
            .iter()
            .chain(source.iter())
            .copied()
            .collect();
        Self::bytes_to_base58(&concat)
    }

    fn indices_to_alphabet(indices: &[u8]) -> String {
        let mut output = String::new();
        for i in indices {
            output.push(Self::ALPHABET.as_bytes()[*i as usize] as char)
        }
        output
    }
}
