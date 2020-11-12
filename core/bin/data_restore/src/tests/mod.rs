pub(crate) mod utils;

use futures::future;
use jsonrpc_core::{Params, Version};
use serde_json::{json, Value};
use std::future::Future;
use web3::{RequestId, Transport};

use db_test_macro::test as db_test;
use zksync_storage::{test_utils::create_data_restore_data, ConnectionPool, StorageProcessor};
use zksync_types::{
    Deposit, DepositOp, ExecutedOperations, ExecutedPriorityOp, ExecutedTx, Log, PriorityOp,
    Withdraw, WithdrawOp, ZkSyncOp, H256,
};

use crate::data_restore_driver::DataRestoreDriver;
use crate::tests::utils::{create_log, u32_to_32bytes};
use crate::{END_ETH_BLOCKS_OFFSET, ETH_BLOCKS_STEP};
use chrono::Utc;
use ethabi::{ParamType, Token};
use std::collections::HashMap;
use web3::contract::tokens::{Tokenizable, Tokenize};
use web3::types::{Bytes, Filter, Transaction, U256};
use zksync_contracts::{governance_contract, zksync_contract};
use zksync_crypto::convert::FeConvert;
use zksync_crypto::{Engine, Fr};
use zksync_storage::test_utils::create_eth;
use zksync_types::block::Block;

#[derive(Debug, Clone)]
pub(crate) struct Web3Transport {
    transactions: HashMap<String, Transaction>,
}
impl Web3Transport {
    fn new() -> Self {
        let mut transactions = HashMap::default();
        let deposit_op = ZkSyncOp::Deposit(Box::new(DepositOp {
            priority_op: Deposit {
                from: Default::default(),
                token: 0,
                amount: 100u32.into(),
                to: Default::default(),
            },
            account_id: 0,
        }));
        let priority_operation = PriorityOp {
            serial_id: 0,
            data: deposit_op.try_get_priority_op().unwrap(),
            deadline_block: 0,
            eth_hash: vec![],
            eth_block: 0,
        };
        let executed_deposit_op = ExecutedPriorityOp {
            priority_op: priority_operation,
            op: deposit_op,
            block_index: 0,
            created_at: Utc::now(),
        };

        let operation = ExecutedOperations::PriorityOp(Box::new(executed_deposit_op));

        let block = Block::new(
            88,
            Fr::default(),
            0,
            vec![operation],
            (0, 0),
            100,
            1_000_000.into(),
            1_500_000.into(),
        );

        let hash: H256 = u32_to_32bytes(1).into();
        let root = block.get_eth_encoded_root();
        let public_data = block.get_eth_public_data();
        let witness_data = block.get_eth_witness_data();
        let fake_data = [0u8; 4];
        let params = (
            u64::from(block.block_number),
            u64::from(block.fee_account),
            vec![root],
            public_data,
            witness_data.0,
            witness_data.1,
        );
        let mut input_data = vec![];
        input_data.extend_from_slice(&fake_data);
        input_data.extend_from_slice(&ethabi::encode(params.into_tokens().as_ref()));

        let tr = Transaction {
            hash: hash.into(),
            nonce: u32_to_32bytes(1).into(),
            block_hash: Some(u32_to_32bytes(100).into()),
            block_number: Some(88.into()),
            transaction_index: Some(block.block_number.into()),
            from: [5u8; 20].into(),
            to: Some([7u8; 20].into()),
            value: u32_to_32bytes(10).into(),
            gas_price: u32_to_32bytes(1).into(),
            gas: u32_to_32bytes(1).into(),
            input: Bytes(input_data),
            raw: None,
        };
        transactions.insert(json!(hash).to_string(), tr);
        Self { transactions }
    }
}

impl Transport for Web3Transport {
    type Out = Box<dyn Future<Output = Result<jsonrpc_core::Value, web3::Error>> + Send + Unpin>;

    fn prepare(
        &self,
        method: &str,
        params: Vec<jsonrpc_core::Value>,
    ) -> (RequestId, jsonrpc_core::Call) {
        (
            1,
            jsonrpc_core::Call::MethodCall(jsonrpc_core::MethodCall {
                jsonrpc: Some(jsonrpc_core::Version::V2),
                method: method.to_string(),
                params: jsonrpc_core::Params::Array(params),
                id: jsonrpc_core::Id::Num(1),
            }),
        )
    }

    fn send(&self, _id: RequestId, request: jsonrpc_core::Call) -> Self::Out {
        Box::new(future::ready({
            if let jsonrpc_core::Call::MethodCall(req) = request {
                let mut params = if let Params::Array(mut params) = req.params {
                    params
                } else {
                    unreachable!()
                };
                match req.method.as_str() {
                    "eth_blockNumber" => Ok(json!("0x80")),
                    "eth_getLogs" => {
                        let filter = params.pop().unwrap();
                        Ok(json!(get_logs(filter)))
                    }
                    "eth_getTransactionByHash" => {
                        let hash = params.pop().unwrap().to_string();
                        if let Some(transaction) = self.transactions.get(&hash) {
                            Ok(json!(transaction))
                        } else {
                            unreachable!()
                        }
                    }
                    "eth_call" => Ok(json!(
                        "0x0000000000000000000000000000000000000000000000000000000000000001"
                    )),
                    _ => Err(web3::Error::Unreachable),
                }
            } else {
                Err(web3::Error::Unreachable)
            }
        }))
    }
}

fn get_logs(filter: Value) -> Vec<Log> {
    let contract = zksync_contract();
    let gov_contract = governance_contract();
    let block_verified_topic = contract
        .event("BlockVerification")
        .expect("Main contract abi error")
        .signature();
    let block_verified_topic_string = &json!(block_verified_topic).to_string()[1..67].to_string();

    let block_committed_topic = contract
        .event("BlockCommit")
        .expect("Main contract abi error")
        .signature();
    let block_commit_topic_string = &json!(block_committed_topic).to_string()[1..67].to_string();

    let reverted_topic = contract
        .event("BlocksRevert")
        .expect("Main contract abi error")
        .signature();

    let reverted_topic_string = &json!(reverted_topic).to_string()[1..67].to_string();

    let new_token_topic = gov_contract
        .event("NewToken")
        .expect("Main contract abi error")
        .signature();
    let new_token_topic_string = &json!(new_token_topic).to_string()[1..67].to_string();

    let from_block = filter.get("fromBlock").unwrap().as_str().unwrap();
    let to_block = filter.get("toBlock").unwrap().as_str().unwrap();

    let topics = if let Ok(topics) =
        serde_json::from_value::<Vec<Vec<String>>>(filter.get("topics").unwrap().clone())
    {
        topics.first().unwrap().clone()
    } else {
        serde_json::from_value::<Vec<String>>(filter.get("topics").unwrap().clone()).unwrap()
    };
    let mut logs = vec![];

    if topics.contains(block_commit_topic_string) {
        logs.push(create_log(
            block_committed_topic.clone(),
            vec![u32_to_32bytes(10).into()],
            Bytes(vec![]),
            10,
        ))
    }
    if topics.contains(block_verified_topic_string) {
        logs.push(create_log(
            block_verified_topic.clone(),
            vec![u32_to_32bytes(10).into()],
            Bytes(vec![]),
            10,
        ))
    }
    if topics.contains(new_token_topic_string) {
        logs.push(create_log(
            new_token_topic.clone(),
            vec![[0; 32].into(), u32_to_32bytes(10).into()],
            Bytes(vec![]),
            10,
        ))
    }
    logs
}

#[db_test]
async fn test_run_state_update(mut storage: StorageProcessor<'_>) {
    create_eth(&mut storage).await;
    let transport = Web3Transport::new();
    let mut driver = DataRestoreDriver::new(
        transport,
        [1u8; 20].into(),
        [1u8; 20].into(),
        ETH_BLOCKS_STEP,
        END_ETH_BLOCKS_OFFSET,
        vec![6, 30],
        true,
        None,
    );
    driver.run_state_update(&mut storage).await;
}
