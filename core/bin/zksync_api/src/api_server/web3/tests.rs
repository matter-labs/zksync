// Built-in uses
use std::str::FromStr;
// External uses
use futures01::future::Future;
use jsonrpc_core::{IoHandler, Params};
use jsonrpc_core_client::{RawClient, RpcError};
use serde_json::Value;
// Workspace uses
use zksync_storage::ConnectionPool;
use zksync_types::BlockNumber;
// Local uses
use super::{
    types::{BlockInfo, Transaction, H160, H256, U256, U64},
    Web3RpcApp,
};
use crate::api_server::rest::v02::test_utils::TestServerConfig;

async fn local_client() -> anyhow::Result<(RawClient, impl Future<Item = (), Error = RpcError>)> {
    let cfg = TestServerConfig::default();
    cfg.fill_database().await?;

    let rpc_app = Web3RpcApp::new(cfg.pool, 9);
    let mut io = IoHandler::new();
    rpc_app.extend(&mut io);

    Ok(jsonrpc_core_client::transports::local::connect::<
        RawClient,
        _,
        _,
    >(io))
}

/// Checks that static methods return values they should return
#[tokio::test(threaded_scheduler)]
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn static_methods() -> anyhow::Result<()> {
    let fut = {
        let (client, server) = local_client().await?;
        let web3_client_version = client.call_method("web3_clientVersion", Params::None);
        let net_version = client.call_method("net_version", Params::None);
        let protocol_version = client.call_method("eth_protocolVersion", Params::None);
        let mining = client.call_method("eth_mining", Params::None);
        let hashrate = client.call_method("eth_hashrate", Params::None);
        let gas_price = client.call_method("eth_gasPrice", Params::None);
        let accounts = client.call_method("eth_accounts", Params::None);
        let get_uncle_count_by_block_hash = client.call_method(
            "eth_getUncleCountByBlockHash",
            Params::Array(vec![serde_json::to_value(H256::zero()).unwrap()]),
        );
        let get_uncle_count_by_block_number = client.call_method(
            "eth_getUncleCountByBlockNumber",
            Params::Array(vec![serde_json::to_value(U64::zero()).unwrap()]),
        );
        let first_join = web3_client_version.join5(net_version, protocol_version, mining, hashrate);
        let second_join = gas_price.join5(
            accounts,
            get_uncle_count_by_block_hash,
            get_uncle_count_by_block_number,
            server,
        );
        first_join.join(second_join)
    };
    let (
        (web3_client_version, net_version, protocol_version, mining, hashrate),
        (gas_price, accounts, get_uncle_count_by_block_hash, get_uncle_count_by_block_number, _),
    ) = fut.wait().unwrap();
    assert_eq!(web3_client_version.as_str().unwrap(), "zkSync");
    assert_eq!(net_version.as_str().unwrap(), "9");
    assert_eq!(protocol_version.as_str().unwrap(), "0");
    assert_eq!(mining.as_bool().unwrap(), false);
    assert_eq!(hashrate.as_str().unwrap(), "0x0");
    assert_eq!(gas_price.as_str().unwrap(), "0x0");
    assert!(accounts.as_array().unwrap().is_empty());
    assert_eq!(get_uncle_count_by_block_hash.as_str().unwrap(), "0x0");
    assert_eq!(get_uncle_count_by_block_number.as_str().unwrap(), "0x0");
    Ok(())
}

/// Tests `eth_blockNumber` method
#[tokio::test(threaded_scheduler)]
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn block_number() -> anyhow::Result<()> {
    let pool = ConnectionPool::new(Some(1));
    // Checks that `eth_blockNumber` return last finalized confirmed block.
    let fut = {
        let (client, server) = local_client().await?;
        client
            .call_method("eth_blockNumber", Params::None)
            .join(server)
    };
    let (block_number, _) = fut.wait().unwrap();
    let expected_block_number = {
        let mut storage = pool.access_storage().await?;
        let block_number = storage
            .chain()
            .block_schema()
            .get_last_verified_confirmed_block()
            .await?;
        U64::from(block_number.0)
    };
    assert_eq!(
        serde_json::from_value::<U64>(block_number).unwrap(),
        expected_block_number
    );
    Ok(())
}

/// Tests `eth_getBalance` method
#[tokio::test(threaded_scheduler)]
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn get_balance() -> anyhow::Result<()> {
    let pool = ConnectionPool::new(Some(1));
    let address = H160::from_str("09d1ef5f45cfa30225edff40cebf657b4226b27b").unwrap();
    // Checks that balance of the account is zero after block with number 0.
    let fut = {
        let (client, server) = local_client().await?;
        client
            .call_method(
                "eth_getBalance",
                Params::Array(vec![
                    Value::String(format!("{:#?}", address)),
                    Value::String("earliest".to_string()),
                ]),
            )
            .join(server)
    };
    let (earliest_balance, _) = fut.wait().unwrap();
    assert_eq!(earliest_balance.as_str().unwrap(), "0x0");

    // Checks that balance of the account equals expected balance after block with number 3.
    let fut = {
        let (client, server) = local_client().await?;
        client
            .call_method(
                "eth_getBalance",
                Params::Array(vec![
                    Value::String("0x09d1ef5f45cfa30225edff40cebf657b4226b27b".to_string()),
                    Value::String("0x3".to_string()),
                ]),
            )
            .join(server)
    };
    let (balance_by_number, _) = fut.wait().unwrap();
    let expected_balance = {
        let mut storage = pool.access_storage().await?;
        let balance = storage
            .chain()
            .account_schema()
            .get_account_eth_balance_for_block(address, BlockNumber(3))
            .await?;
        U256::from_dec_str(&balance.to_string()).unwrap()
    };
    assert_eq!(
        serde_json::from_value::<U256>(balance_by_number).unwrap(),
        expected_balance
    );

    // Checks that balance of the account equals expected balance after the last block.
    let fut = {
        let (client, server) = local_client().await?;
        client
            .call_method(
                "eth_getBalance",
                Params::Array(vec![Value::String(
                    "0x09d1ef5f45cfa30225edff40cebf657b4226b27b".to_string(),
                )]),
            )
            .join(server)
    };
    let (latest_balance, _) = fut.wait().unwrap();
    let expected_balance = {
        let mut storage = pool.access_storage().await?;
        let last_block = storage
            .chain()
            .block_schema()
            .get_last_saved_block()
            .await?;
        let balance = storage
            .chain()
            .account_schema()
            .get_account_eth_balance_for_block(address, last_block)
            .await?;
        U256::from_dec_str(&balance.to_string()).unwrap()
    };
    assert_eq!(
        serde_json::from_value::<U256>(latest_balance).unwrap(),
        expected_balance
    );

    Ok(())
}

/// Tests `eth_getBlockTransactionCountByHash` and `eth_getBlockTransactionCountByNumber` methods
#[tokio::test(threaded_scheduler)]
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn get_block_transaction_count() -> anyhow::Result<()> {
    let pool = ConnectionPool::new(Some(1));
    // Checks that `eth_getBlockTransactionCountByHash` works correctly.
    let fut = {
        let (client, server) = local_client().await?;
        client
            .call_method(
                "eth_getBlockTransactionCountByHash",
                Params::Array(vec![Value::String(
                    "0x0000000000000000000000000000000000000000000000000000000000000001"
                        .to_string(),
                )]),
            )
            .join(server)
    };
    let (count_by_hash, _) = fut.wait().unwrap();
    let expected = {
        let mut storage = pool.access_storage().await?;
        U256::from(
            storage
                .chain()
                .block_schema()
                .get_block_transactions_count(BlockNumber(1))
                .await?,
        )
    };
    assert_eq!(
        serde_json::from_value::<U256>(count_by_hash).unwrap(),
        expected
    );

    // Checks that `eth_getBlockTransactionCountByNumber` works correctly for provided block.
    let fut = {
        let (client, server) = local_client().await?;
        client
            .call_method(
                "eth_getBlockTransactionCountByNumber",
                Params::Array(vec![Value::String("0x1".to_string())]),
            )
            .join(server)
    };
    let (count_by_number, _) = fut.wait().unwrap();
    assert_eq!(
        serde_json::from_value::<U256>(count_by_number).unwrap(),
        expected
    );

    // Checks that `eth_getBlockTransactionCountByNumber` works correctly for the last block.
    let fut = {
        let (client, server) = local_client().await?;
        client
            .call_method("eth_getBlockTransactionCountByNumber", Params::None)
            .join(server)
    };
    let (count_in_last_block, _) = fut.wait().unwrap();
    let expected = {
        let mut storage = pool.access_storage().await?;
        let last_block = storage
            .chain()
            .block_schema()
            .get_last_saved_block()
            .await?;
        U256::from(
            storage
                .chain()
                .block_schema()
                .get_block_transactions_count(last_block)
                .await?,
        )
    };
    assert_eq!(
        serde_json::from_value::<U256>(count_in_last_block).unwrap(),
        expected
    );

    Ok(())
}

/// Tests `eth_getTransactionByHash` methods
#[tokio::test(threaded_scheduler)]
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn get_transaction_by_hash() -> anyhow::Result<()> {
    let pool = ConnectionPool::new(Some(1));
    // Checks that `eth_getTransactionByHash` returns `null` for non-existent transaction hash.
    let fut = {
        let (client, server) = local_client().await?;
        client
            .call_method(
                "eth_getTransactionByHash",
                Params::Array(vec![Value::String(
                    "0x0000000000000000000000000000000000000000000000000000000000000000"
                        .to_string(),
                )]),
            )
            .join(server)
    };
    let (transaction, _) = fut.wait().unwrap();
    assert!(transaction.is_null());

    // Checks that `eth_getTransactionByHash` works correctly for existent transaction.
    let tx_hash = {
        let mut storage = pool.access_storage().await?;
        storage
            .chain()
            .block_schema()
            .get_block_transactions_hashes(BlockNumber(1))
            .await?
            .remove(0)
    };
    let tx_hash_str = format!("0x{}", hex::encode(&tx_hash));
    let fut = {
        let (client, server) = local_client().await?;
        client
            .call_method(
                "eth_getTransactionByHash",
                Params::Array(vec![Value::String(tx_hash_str)]),
            )
            .join(server)
    };
    let (transaction, _) = fut.wait().unwrap();
    let expected = {
        let mut storage = pool.access_storage().await?;
        let tx_data = storage
            .chain()
            .operations_ext_schema()
            .tx_data_for_web3(&tx_hash)
            .await?
            .unwrap();
        Web3RpcApp::transaction_from_tx_data(tx_data.into())
    };
    assert_eq!(
        serde_json::from_value::<Transaction>(transaction).unwrap(),
        expected
    );

    Ok(())
}

/// Tests `eth_getBlockByNumber` and `eth_getBlockByHash` methods
#[tokio::test(threaded_scheduler)]
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn get_block() -> anyhow::Result<()> {
    let pool = ConnectionPool::new(Some(1));
    // Checks that `eth_getBlockByHash` returns `null` for non-existent transaction hash.
    let fut = {
        let (client, server) = local_client().await?;
        client
            .call_method(
                "eth_getBlockByHash",
                Params::Array(vec![
                    Value::String(
                        "0xdeadbeef00000000000000000000000000000000000000000000000000000000"
                            .to_string(),
                    ),
                    Value::Bool(false),
                ]),
            )
            .join(server)
    };
    let (block, _) = fut.wait().unwrap();
    assert!(block.is_null());

    // Checks that `eth_getBlockByHash` returns correct block with tx hashes.
    let fut = {
        let (client, server) = local_client().await?;
        client
            .call_method(
                "eth_getBlockByHash",
                Params::Array(vec![
                    Value::String(
                        "0x0000000000000000000000000000000000000000000000000000000000000002"
                            .to_string(),
                    ),
                    Value::Bool(false),
                ]),
            )
            .join(server)
    };
    let (block, _) = fut.wait().unwrap();
    let expected = {
        let mut storage = pool.access_storage().await?;
        Web3RpcApp::block_by_number(&mut storage, BlockNumber(2), false).await?
    };
    assert_eq!(
        serde_json::from_value::<BlockInfo>(block).unwrap(),
        expected
    );

    // Checks that `eth_getBlockByNumber` returns correct block with txs.
    let fut = {
        let (client, server) = local_client().await?;
        client
            .call_method(
                "eth_getBlockByNumber",
                Params::Array(vec![Value::String("0x2".to_string()), Value::Bool(true)]),
            )
            .join(server)
    };
    let (block, _) = fut.wait().unwrap();
    let expected = {
        let mut storage = pool.access_storage().await?;
        Web3RpcApp::block_by_number(&mut storage, BlockNumber(2), true).await?
    };
    assert_eq!(
        serde_json::from_value::<BlockInfo>(block).unwrap(),
        expected
    );

    Ok(())
}
