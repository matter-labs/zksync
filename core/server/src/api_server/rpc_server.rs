use crate::ThreadPanicNotify;
use futures::Future;
use jsonrpc_core::IoHandler;
use jsonrpc_core::{Error, Result};
use jsonrpc_derive::rpc;
use jsonrpc_http_server::ServerBuilder;
use models::node::{Account, AccountAddress, AccountId, FranklinTx};
use std::sync::mpsc;
use storage::{ConnectionPool, StorageProcessor, Token, TxAddError};

#[derive(Serialize, Deserialize)]
pub struct AccountInfoResp {
    id: Option<AccountId>,
    commited: Account,
    verified: Account,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct BlockInfo {
    pub block_number: i64,
    pub commited: bool,
    pub verified: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionInfoResp {
    pub executed: bool,
    pub success: Option<bool>,
    pub fail_reason: Option<String>,
    pub block: Option<BlockInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ETHOpInfoResp {
    pub executed: bool,
    pub block: Option<BlockInfo>,
}

#[rpc]
pub trait Rpc {
    #[rpc(name = "account_info")]
    fn account_info(&self, addr: AccountAddress) -> Result<AccountInfoResp>;
    #[rpc(name = "ethop_info")]
    fn ethop_info(&self, serial_id: u32) -> Result<ETHOpInfoResp>;
    #[rpc(name = "tx_info")]
    fn tx_info(&self, hash: String) -> Result<TransactionInfoResp>;
    #[rpc(name = "tx_submit")]
    fn tx_submit(&self, tx: FranklinTx) -> Result<String>;
    #[rpc(name = "chain_tokens")]
    fn chain_tokens(&self) -> Result<Vec<Token>>;
}

struct RpcApp {
    connection_pool: ConnectionPool,
}

impl RpcApp {
    fn access_storage(&self) -> Result<StorageProcessor> {
        self.connection_pool
            .access_storage()
            .map_err(|_| Error::internal_error())
    }
}

impl Rpc for RpcApp {
    fn account_info(&self, addr: AccountAddress) -> Result<AccountInfoResp> {
        let storage = self.access_storage()?;
        let account = storage
            .account_state_by_address(&addr)
            .map_err(|_| Error::internal_error())?;
        let default_account = || Account::default_with_address(&addr);
        Ok(AccountInfoResp {
            id: account.commited.as_ref().map(|(id, _)| *id),
            commited: account
                .commited
                .map(|(_, acc)| acc)
                .unwrap_or_else(default_account),
            verified: account
                .verified
                .map(|(_, acc)| acc)
                .unwrap_or_else(default_account),
        })
    }

    fn ethop_info(&self, serial_id: u32) -> Result<ETHOpInfoResp> {
        let storage = self.access_storage()?;
        let executed_op = storage
            .get_executed_priority_op(serial_id)
            .map_err(|_| Error::internal_error())?;
        Ok(if let Some(executed_op) = executed_op {
            let block = storage.handle_search(executed_op.block_number.to_string());
            ETHOpInfoResp {
                executed: true,
                block: Some(BlockInfo {
                    block_number: executed_op.block_number,
                    commited: true,
                    verified: block.map(|b| b.verified_at.is_some()).unwrap_or_default(),
                }),
            }
        } else {
            ETHOpInfoResp {
                executed: false,
                block: None,
            }
        })
    }

    fn tx_info(&self, tx_hash: String) -> Result<TransactionInfoResp> {
        let hash = decode_hash(&tx_hash)?;
        let storage = self.access_storage()?;
        let stored_receipt = storage
            .tx_receipt(&hash)
            .map_err(|_| Error::internal_error())?;
        Ok(if let Some(stored_receipt) = stored_receipt {
            TransactionInfoResp {
                executed: true,
                success: Some(stored_receipt.success),
                fail_reason: stored_receipt.fail_reason,
                block: Some(BlockInfo {
                    block_number: stored_receipt.block_number,
                    commited: true,
                    verified: stored_receipt.verified,
                }),
            }
        } else {
            TransactionInfoResp {
                executed: false,
                success: None,
                fail_reason: None,
                block: None,
            }
        })
    }

    fn tx_submit(&self, tx: FranklinTx) -> Result<String> {
        let hash = hex::encode(tx.hash().as_ref());
        let storage = self.access_storage()?;

        let tx_add_result = storage
            .mempool_add_tx(&tx)
            .map_err(|_| Error::internal_error())?;

        tx_add_result.map(|_| hash).map_err(|e| {
            let code = match &e {
                TxAddError::NonceTooLow => 101,
                TxAddError::InvalidSignature => 102,
                TxAddError::IncorrectTx => 103,
            };
            Error {
                code: code.into(),
                message: e.to_string(),
                data: None,
            }
        })
    }

    fn chain_tokens(&self) -> Result<Vec<Token>> {
        Ok(self
            .access_storage()?
            .load_tokens()
            .map_err(|_| Error::internal_error())?)
    }
}

fn decode_hash(hash: &str) -> Result<Vec<u8>> {
    let vec = hex::decode(hash).map_err(|e| Error::invalid_params(e.to_string()))?;
    if vec.len() == 32 {
        Ok(vec)
    } else {
        Err(Error::invalid_params("hash len mismatch"))
    }
}

pub fn start_rpc_server(connection_pool: ConnectionPool, panic_notify: mpsc::Sender<bool>) {
    std::thread::Builder::new()
        .name("json_rpc".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);
            let mut io = IoHandler::new();

            let rpc_app = RpcApp { connection_pool };
            io.extend_with(rpc_app.to_delegate());

            let server = ServerBuilder::new(io)
                .threads(1)
                .start_http(&"127.0.0.1:3030".parse().unwrap())
                .unwrap();

            server.wait();
        })
        .expect("JSON rpc thread");
}
