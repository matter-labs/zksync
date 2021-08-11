// Built-in uses
use std::str::FromStr;
// External uses
use futures::future::{join, join5, Future};
use jsonrpc_core::{Error, ErrorCode, IoHandler, Params};
use jsonrpc_core_client::{RawClient, RpcError, RpcResult};
use num::BigUint;
use serde_json::{Map, Value};
// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_storage::{chain::operations_ext::records::Web3TxReceipt, ConnectionPool};
use zksync_test_account::ZkSyncAccount;
use zksync_types::{
    tx::ChangePubKeyType, AccountId, AccountUpdate, BlockNumber, ChangePubKeyOp, CloseOp, Deposit,
    DepositOp, ForcedExitOp, FullExit, FullExitOp, MintNFTOp, Nonce, SwapOp, TokenId, TransferOp,
    WithdrawNFTOp, WithdrawOp, ZkSyncOp, NFT,
};
// Local uses
use super::{
    converter::transaction_from_tx_data,
    types::{BlockInfo, Event, Log, Transaction, TransactionReceipt, H160, H256, U256, U64},
    Web3RpcApp,
};
use crate::api_server::rest::v02::test_utils::TestServerConfig;

async fn local_client() -> anyhow::Result<(RawClient, impl Future<Output = RpcResult<()>>)> {
    let cfg = TestServerConfig::default();
    cfg.fill_database().await?;

    let rpc_app = Web3RpcApp::new(cfg.pool, &cfg.config);
    let mut io = IoHandler::new();
    rpc_app.extend(&mut io);

    Ok(jsonrpc_core_client::transports::local::connect::<
        RawClient,
        _,
        _,
    >(io))
}

/// Checks that static methods return values they should return
#[tokio::test(flavor = "multi_thread")]
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
        let first_join = join5(
            web3_client_version,
            net_version,
            protocol_version,
            mining,
            hashrate,
        );
        let second_join = join5(
            gas_price,
            accounts,
            get_uncle_count_by_block_hash,
            get_uncle_count_by_block_number,
            server,
        );

        join(first_join, second_join)
    };
    let (
        (web3_client_version, net_version, protocol_version, mining, hashrate),
        (gas_price, accounts, get_uncle_count_by_block_hash, get_uncle_count_by_block_number, _),
    ) = fut.await;
    assert_eq!(web3_client_version.unwrap().as_str().unwrap(), "zkSync");
    assert_eq!(net_version.unwrap().as_str().unwrap(), "9");
    assert_eq!(protocol_version.unwrap().as_str().unwrap(), "0");
    assert_eq!(mining.unwrap().as_bool().unwrap(), false);
    assert_eq!(hashrate.unwrap().as_str().unwrap(), "0x0");
    assert_eq!(gas_price.unwrap().as_str().unwrap(), "0x0");
    assert!(accounts.unwrap().as_array().unwrap().is_empty());
    assert_eq!(
        get_uncle_count_by_block_hash.unwrap().as_str().unwrap(),
        "0x0"
    );
    assert_eq!(
        get_uncle_count_by_block_number.unwrap().as_str().unwrap(),
        "0x0"
    );
    Ok(())
}

/// Tests `eth_blockNumber` method
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn block_number() -> anyhow::Result<()> {
    let pool = ConnectionPool::new(Some(1));
    // Checks that `eth_blockNumber` return last finalized confirmed block.
    let fut = {
        let (client, server) = local_client().await?;
        join(client.call_method("eth_blockNumber", Params::None), server)
    };
    let block_number = fut.await.0.unwrap();
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
#[tokio::test(flavor = "multi_thread")]
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
        join(
            client.call_method(
                "eth_getBalance",
                Params::Array(vec![
                    Value::String(format!("{:#?}", address)),
                    Value::String("earliest".to_string()),
                ]),
            ),
            server,
        )
    };
    let earliest_balance = fut.await.0.unwrap();
    assert_eq!(earliest_balance.as_str().unwrap(), "0x0");

    // Checks that balance of the account equals expected balance after block with number 3.
    let fut = {
        let (client, server) = local_client().await?;
        join(
            client.call_method(
                "eth_getBalance",
                Params::Array(vec![
                    Value::String("0x09d1ef5f45cfa30225edff40cebf657b4226b27b".to_string()),
                    Value::String("0x3".to_string()),
                ]),
            ),
            server,
        )
    };
    let balance_by_number = fut.await.0.unwrap();
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
        join(
            client.call_method(
                "eth_getBalance",
                Params::Array(vec![Value::String(
                    "0x09d1ef5f45cfa30225edff40cebf657b4226b27b".to_string(),
                )]),
            ),
            server,
        )
    };
    let latest_balance = fut.await.0.unwrap();
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
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn get_block_transaction_count() -> anyhow::Result<()> {
    let pool = ConnectionPool::new(Some(1));
    // Checks that `eth_getBlockTransactionCountByHash` works correctly.
    let fut = {
        let (client, server) = local_client().await?;
        join(
            client.call_method(
                "eth_getBlockTransactionCountByHash",
                Params::Array(vec![Value::String(
                    "0x0000000000000000000000000000000000000000000000000000000000000001"
                        .to_string(),
                )]),
            ),
            server,
        )
    };
    let count_by_hash = fut.await.0.unwrap();
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
        join(
            client.call_method(
                "eth_getBlockTransactionCountByNumber",
                Params::Array(vec![Value::String("0x1".to_string())]),
            ),
            server,
        )
    };
    let count_by_number = fut.await.0.unwrap();
    assert_eq!(
        serde_json::from_value::<U256>(count_by_number).unwrap(),
        expected
    );

    // Checks that `eth_getBlockTransactionCountByNumber` works correctly for the last block.
    let fut = {
        let (client, server) = local_client().await?;
        join(
            client.call_method("eth_getBlockTransactionCountByNumber", Params::None),
            server,
        )
    };
    let count_in_last_block = fut.await.0.unwrap();
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
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn get_transaction_by_hash() -> anyhow::Result<()> {
    let pool = ConnectionPool::new(Some(1));
    // Checks that `eth_getTransactionByHash` returns `null` for non-existent transaction hash.
    let fut = {
        let (client, server) = local_client().await?;
        join(
            client.call_method(
                "eth_getTransactionByHash",
                Params::Array(vec![Value::String(
                    "0x0000000000000000000000000000000000000000000000000000000000000000"
                        .to_string(),
                )]),
            ),
            server,
        )
    };
    let transaction = fut.await.0.unwrap();
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
        join(
            client.call_method(
                "eth_getTransactionByHash",
                Params::Array(vec![Value::String(tx_hash_str)]),
            ),
            server,
        )
    };
    let transaction = fut.await.0.unwrap();
    let expected = {
        let mut storage = pool.access_storage().await?;
        let tx_data = storage
            .chain()
            .operations_ext_schema()
            .tx_data_for_web3(&tx_hash)
            .await?
            .unwrap();
        transaction_from_tx_data(tx_data.into())
    };
    assert_eq!(
        serde_json::from_value::<Transaction>(transaction).unwrap(),
        expected
    );

    Ok(())
}

/// Tests `eth_getBlockByNumber` and `eth_getBlockByHash` methods
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn get_block() -> anyhow::Result<()> {
    let pool = ConnectionPool::new(Some(1));
    // Checks that `eth_getBlockByHash` returns `null` for non-existent transaction hash.
    let fut = {
        let (client, server) = local_client().await?;
        join(
            client.call_method(
                "eth_getBlockByHash",
                Params::Array(vec![
                    Value::String(
                        "0xdeadbeef00000000000000000000000000000000000000000000000000000000"
                            .to_string(),
                    ),
                    Value::Bool(false),
                ]),
            ),
            server,
        )
    };
    let block = fut.await.0.unwrap();
    assert!(block.is_null());

    // Checks that `eth_getBlockByHash` returns correct block with tx hashes.
    let fut = {
        let (client, server) = local_client().await?;
        join(
            client.call_method(
                "eth_getBlockByHash",
                Params::Array(vec![
                    Value::String(
                        "0x0000000000000000000000000000000000000000000000000000000000000002"
                            .to_string(),
                    ),
                    Value::Bool(false),
                ]),
            ),
            server,
        )
    };
    let block = fut.await.0.unwrap();
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
        join(
            client.call_method(
                "eth_getBlockByNumber",
                Params::Array(vec![Value::String("0x2".to_string()), Value::Bool(true)]),
            ),
            server,
        )
    };
    let block = fut.await.0.unwrap();
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

/// Tests creating logs from transactions
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn create_logs() -> anyhow::Result<()> {
    let cfg = TestServerConfig::default();
    cfg.fill_database().await?;
    let rpc_app = Web3RpcApp::new(cfg.pool, &cfg.config);

    let from_account_id = AccountId(3);
    let from_account = ZkSyncAccount::rand_with_seed([1, 2, 3, 4]);
    from_account.set_account_id(Some(from_account_id));

    let to_account_id = AccountId(1474183);
    let to_account = ZkSyncAccount::rand_with_seed([5, 6, 7, 8]);
    to_account.set_account_id(Some(to_account_id));

    let mut storage = rpc_app.connection_pool.access_storage().await?;

    let token1 = storage
        .tokens_schema()
        .get_token(TokenId(0).into())
        .await?
        .unwrap();
    let token2 = storage
        .tokens_schema()
        .get_token(TokenId(1).into())
        .await?
        .unwrap();
    let nft = storage
        .tokens_schema()
        .get_nft(TokenId(65544))
        .await?
        .unwrap();

    let amount = BigUint::from(100u32);
    let fee = BigUint::from(1u32);

    let transfer_op = {
        let tx = from_account
            .sign_transfer(
                token1.id,
                &token1.symbol,
                amount.clone(),
                fee.clone(),
                &to_account.address,
                None,
                true,
                Default::default(),
            )
            .0;
        TransferOp {
            tx,
            from: from_account_id,
            to: to_account_id,
        }
    };
    let withdraw_op = {
        let tx = from_account
            .sign_withdraw(
                token1.id,
                &token1.symbol,
                amount.clone(),
                fee.clone(),
                &to_account.address,
                None,
                true,
                Default::default(),
            )
            .0;
        WithdrawOp {
            tx,
            account_id: from_account_id,
        }
    };
    let forced_exit_op = {
        let tx = from_account.sign_forced_exit(
            token1.id,
            fee.clone(),
            &to_account.address,
            None,
            true,
            Default::default(),
        );
        ForcedExitOp {
            tx,
            target_account_id: from_account_id,
            withdraw_amount: Some(amount.clone().into()),
        }
    };
    let change_pub_key_op = {
        let tx = from_account.sign_change_pubkey_tx(
            None,
            true,
            token1.id,
            fee.clone(),
            ChangePubKeyType::ECDSA,
            Default::default(),
        );
        ChangePubKeyOp {
            tx,
            account_id: from_account_id,
        }
    };
    let mint_nft_op = {
        let tx = from_account
            .sign_mint_nft(
                token1.id,
                &token1.symbol,
                H256::from_str("aaaa00000000000000000000000000000000000000000000000000000000bbbb")
                    .unwrap(),
                fee.clone(),
                &to_account.address,
                Some(Nonce(10)),
                true,
            )
            .0;
        MintNFTOp {
            tx,
            creator_account_id: from_account_id,
            recipient_account_id: to_account_id,
        }
    };
    let created_nft = NFT::new(
        TokenId(71234),
        1,
        from_account_id,
        from_account.address,
        H160::from_str("abcd000000000000000000000000000000000000").unwrap(),
        None,
        H256::zero(),
    );
    let update = (
        from_account_id,
        AccountUpdate::MintNFT {
            token: created_nft,
            nonce: Nonce(10),
        },
    );
    storage
        .chain()
        .state_schema()
        .commit_state_update(BlockNumber(100), &[update], 0)
        .await?;
    let withdraw_nft_op = {
        let tx = from_account
            .sign_withdraw_nft(
                nft.id,
                token1.id,
                &token1.symbol,
                fee.clone(),
                &to_account.address,
                None,
                true,
                Default::default(),
            )
            .0;
        WithdrawNFTOp {
            tx,
            creator_id: from_account_id,
            creator_address: from_account.address,
            serial_id: nft.serial_id,
            content_hash: nft.content_hash,
        }
    };
    let swap_op = {
        let order1 = from_account.sign_order(
            token1.id,
            token2.id,
            1u32.into(),
            1u32.into(),
            amount.clone(),
            &from_account.address,
            None,
            true,
            Default::default(),
        );
        let order2 = to_account.sign_order(
            token2.id,
            token1.id,
            1u32.into(),
            1u32.into(),
            amount.clone(),
            &to_account.address,
            None,
            true,
            Default::default(),
        );
        let tx = from_account
            .sign_swap(
                (order1, order2),
                (amount.clone(), amount.clone()),
                None,
                true,
                token1.id,
                &token1.symbol,
                fee.clone(),
            )
            .0;
        SwapOp {
            tx,
            submitter: from_account_id,
            accounts: (from_account_id, to_account_id),
            recipients: (from_account_id, to_account_id),
        }
    };
    let deposit_op = DepositOp {
        priority_op: Deposit {
            from: from_account.address,
            token: token1.id,
            amount: amount.clone(),
            to: to_account.address,
        },
        account_id: from_account_id,
    };
    let full_exit_op = FullExitOp {
        priority_op: FullExit {
            account_id: from_account_id,
            eth_address: from_account.address,
            token: token1.id,
            is_legacy: false,
        },
        withdraw_amount: Some(amount.clone().into()),
        creator_account_id: None,
        creator_address: None,
        serial_id: None,
        content_hash: None,
    };
    let close_op = {
        let tx = from_account.sign_close(None, true);
        CloseOp {
            tx,
            account_id: from_account_id,
        }
    };

    let test_data: Vec<(ZkSyncOp, _)> = vec![
        (
            transfer_op.into(),
            vec![(Event::ERCTransfer, "000000000000000000000000a3dfe7b9dec5b30369aa5b5e53df47e95294a2d30000000000000000000000006247f65195f37229068af47775fee7355e660e400000000000000000000000000000000000000000000000000000000000000064", "erc transfer for transfer"),
                 (Event::ZkSyncTransfer, "000000000000000000000000a3dfe7b9dec5b30369aa5b5e53df47e95294a2d30000000000000000000000006247f65195f37229068af47775fee7355e660e40000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000001", "zksync transfer")],
        ),
        (
            withdraw_op.into(),
            vec![(Event::ERCTransfer, "000000000000000000000000a3dfe7b9dec5b30369aa5b5e53df47e95294a2d300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064", "erc transfer for withdraw"),
                 (Event::ZkSyncWithdraw, "000000000000000000000000a3dfe7b9dec5b30369aa5b5e53df47e95294a2d30000000000000000000000006247f65195f37229068af47775fee7355e660e40000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000001", "zksync withdraw")],
        ),
        (
            forced_exit_op.into(),
            vec![(Event::ERCTransfer, "00000000000000000000000009d1ef5f45cfa30225edff40cebf657b4226b27b00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064", "erc transfer for forced exit"),
                 (Event::ZkSyncForcedExit, "00000000000000000000000009d1ef5f45cfa30225edff40cebf657b4226b27b0000000000000000000000006247f65195f37229068af47775fee7355e660e4000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001", "zksync forced exit")],
        ),
        (change_pub_key_op.into(), vec![(Event::ZkSyncChangePubKey, "000000000000000000000000a3dfe7b9dec5b30369aa5b5e53df47e95294a2d363aa2a0efb97064e0e52a6adb63a42018bd6e72b00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001", "zksync change pub key")]),
        (
            mint_nft_op.into(),
            vec![(Event::ERCTransfer, "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000006247f65195f37229068af47775fee7355e660e400000000000000000000000000000000000000000000000000000000000011642", "erc transfer for mint nft"),
                (Event::ZkSyncMintNFT, "00000000000000000000000000000000000000000000000000000000000116420000000000000000000000000000000000000000000000000000000000000003000000000000000000000000a3dfe7b9dec5b30369aa5b5e53df47e95294a2d3aaaa00000000000000000000000000000000000000000000000000000000bbbb0000000000000000000000006247f65195f37229068af47775fee7355e660e4000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000", "zksync mint nft")]
        ),
        (
            withdraw_nft_op.into(),
            vec![(Event::ERCTransfer, "000000000000000000000000a3dfe7b9dec5b30369aa5b5e53df47e95294a2d300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010008", "erc transfer for withdraw nft"),
                 (Event::ZkSyncWithdrawNFT, "000000000000000000000000a3dfe7b9dec5b30369aa5b5e53df47e95294a2d30000000000000000000000006247f65195f37229068af47775fee7355e660e400000000000000000000000000000000000000000000000000000000000010008000000000000000000000000092ae2ef6082d989a350ec7e320ff65c36d85c7a000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000167e8700000000000000000000000074d69238e389af5b08a16e7dc79a2fea53c59ffc00000000000000000000000000000000000000000000000000000000000000088a46969af38b0cc2518828b6547e1bfd2b4b29df4141cfb68ed653ebcabdf44b", "zksync withdraw nft")],
        ),
        (
            swap_op.into(),
            vec![(Event::ERCTransfer, "00000000000000000000000009d1ef5f45cfa30225edff40cebf657b4226b27b0000000000000000000000006247f65195f37229068af47775fee7355e660e400000000000000000000000000000000000000000000000000000000000000064", "erc transfer1 for swap"),
                 (Event::ERCTransfer, "00000000000000000000000074d69238e389af5b08a16e7dc79a2fea53c59ffc000000000000000000000000a3dfe7b9dec5b30369aa5b5e53df47e95294a2d30000000000000000000000000000000000000000000000000000000000000064", "erc transfer2 for swap"),
                 (Event::ZkSyncSwap, "000000000000000000000000a3dfe7b9dec5b30369aa5b5e53df47e95294a2d300000000000000000000000009d1ef5f45cfa30225edff40cebf657b4226b27b00000000000000000000000074d69238e389af5b08a16e7dc79a2fea53c59ffc000000000000000000000000a3dfe7b9dec5b30369aa5b5e53df47e95294a2d30000000000000000000000006247f65195f37229068af47775fee7355e660e40000000000000000000000000000000000000000000000000000000000000000000000000000000000000000038a2fdc11f526ddd5a607c1f251c065f40fbf2f70000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000640000000000000000000000000000000000000000000000000000000000000064", "zksync swap")],
        ),
        (
            deposit_op.into(),
            vec![(Event::ERCTransfer, "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000006247f65195f37229068af47775fee7355e660e400000000000000000000000000000000000000000000000000000000000000064", "erc transfer for deposit"),
                 (Event::ZkSyncDeposit, "000000000000000000000000a3dfe7b9dec5b30369aa5b5e53df47e95294a2d30000000000000000000000006247f65195f37229068af47775fee7355e660e4000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064", "zksync deposit")],
        ),
        (
            full_exit_op.into(),
            vec![(Event::ERCTransfer, "00000000000000000000000009d1ef5f45cfa30225edff40cebf657b4226b27b00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064", "erc transfer for full exit"),
                 (Event::ZkSyncFullExit, "00000000000000000000000009d1ef5f45cfa30225edff40cebf657b4226b27b00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000064", "zksync full exit")],
        ),
        (close_op.into(), Vec::new()),
    ];

    // Checks that logs have correct topics and data.
    for (op, events) in test_data {
        let receipt = Web3TxReceipt {
            tx_hash: H256::zero().as_bytes().to_vec(),
            block_number: 0,
            operation: serde_json::to_value(op).unwrap(),
            block_hash: H256::zero().as_bytes().to_vec(),
            block_index: Some(0),
            from_account: H160::zero().as_bytes().to_vec(),
            to_account: Some(H160::zero().as_bytes().to_vec()),
            success: true,
        };
        let logs = rpc_app.logs_from_receipt(&mut storage, receipt).await?;
        assert_eq!(logs.len(), events.len());
        for (idx, (log, (event, data, test_name))) in logs.into_iter().zip(events).enumerate() {
            assert_eq!(log.topics.len(), 1, "{}", test_name);
            assert_eq!(
                log.topics[0],
                rpc_app.logs_helper.topic_by_event(event).unwrap(),
                "{}",
                test_name
            );
            assert_eq!(log.data.0, hex::decode(data).unwrap(), "{}", test_name);
            assert_eq!(log.transaction_log_index.unwrap().as_u32(), idx as u32);
        }
    }

    Ok(())
}

/// Tests `eth_getTransactionReceipt` method
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn get_transaction_receipt() -> anyhow::Result<()> {
    let pool = ConnectionPool::new(Some(1));
    // Checks that `eth_getTransactionReceipt` returns `null` for non-existent transaction hash.
    let fut = {
        let (client, server) = local_client().await?;
        join(
            client.call_method(
                "eth_getTransactionReceipt",
                Params::Array(vec![Value::String(
                    "0xdeadbeef00000000000000000000000000000000000000000000000000000000"
                        .to_string(),
                )]),
            ),
            server,
        )
    };
    let receipt = fut.await.0.unwrap();
    assert!(receipt.is_null());

    // Checks that `eth_getTransactionReceipt` returns correct receipt for an existent transaction.
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
        join(
            client.call_method(
                "eth_getTransactionReceipt",
                Params::Array(vec![Value::String(tx_hash_str)]),
            ),
            server,
        )
    };
    let receipt = fut.await.0.unwrap();
    let expected = {
        let mut storage = pool.access_storage().await?;
        let receipt = storage
            .chain()
            .operations_ext_schema()
            .web3_receipt_by_hash(&tx_hash)
            .await?
            .unwrap();
        let rpc_app = Web3RpcApp::new(pool.clone(), &ZkSyncConfig::from_env());
        rpc_app.tx_receipt(&mut storage, receipt).await?
    };
    assert_eq!(
        serde_json::from_value::<TransactionReceipt>(receipt).unwrap(),
        expected
    );

    Ok(())
}

/// Tests `eth_getLogs` method
#[tokio::test(flavor = "multi_thread")]
#[cfg_attr(
    not(feature = "api_test"),
    ignore = "Use `zk test rust-api` command to perform this test"
)]
async fn get_logs() -> anyhow::Result<()> {
    let pool = ConnectionPool::new(Some(1));
    let rpc_app = Web3RpcApp::new(pool.clone(), &ZkSyncConfig::from_env());

    // Checks that it returns error if `fromBlock` is greater than `toBlock`.
    let fut = {
        let (client, server) = local_client().await?;
        let mut req = Map::new();
        req.insert("fromBlock".to_string(), Value::String("0x2".to_string()));
        req.insert("toBlock".to_string(), Value::String("0x1".to_string()));
        join(
            client.call_method("eth_getLogs", Params::Array(vec![Value::Object(req)])),
            server,
        )
    };
    let error = fut.await.0.unwrap_err();
    assert!(matches!(
        error,
        RpcError::JsonRpcError(Error {
            code: ErrorCode::InvalidParams,
            ..
        })
    ));

    // Checks that it returns error if block range is too big.
    let fut = {
        let (client, server) = {
            let mut config = ZkSyncConfig::from_env();
            config.api.web3.max_block_range = 3;

            let rpc_app = Web3RpcApp::new(pool.clone(), &config);
            let mut io = IoHandler::new();
            rpc_app.extend(&mut io);

            jsonrpc_core_client::transports::local::connect::<RawClient, _, _>(io)
        };
        let mut req = Map::new();
        req.insert("fromBlock".to_string(), Value::String("0x1".to_string()));
        req.insert("toBlock".to_string(), Value::String("0x5".to_string()));
        join(
            client.call_method("eth_getLogs", Params::Array(vec![Value::Object(req)])),
            server,
        )
    };
    let error = fut.await.0.unwrap_err();
    assert!(matches!(
        error,
        RpcError::JsonRpcError(Error {
            code: ErrorCode::InvalidParams,
            ..
        })
    ));

    // Checks that block filter works correctly.
    let fut = {
        let (client, server) = local_client().await?;
        let mut req = Map::new();
        req.insert("fromBlock".to_string(), Value::String("0x1".to_string()));
        req.insert("toBlock".to_string(), Value::String("0x1".to_string()));
        join(
            client.call_method("eth_getLogs", Params::Array(vec![Value::Object(req)])),
            server,
        )
    };
    let logs = fut.await.0.unwrap();
    let logs = serde_json::from_value::<Vec<Log>>(logs).unwrap();
    assert_eq!(logs.len(), 9);
    for log in logs {
        assert_eq!(log.block_number.unwrap().as_u64(), 1);
    }

    // Checks that address filter works correctly
    let mut addresses = Vec::new();
    {
        let mut storage = pool.access_storage().await?;
        let token = storage
            .tokens_schema()
            .get_token(TokenId(0).into())
            .await?
            .unwrap();
        addresses.push(token.address);
    }
    addresses.push(rpc_app.logs_helper.zksync_proxy_address);

    let fut = {
        let (client, server) = local_client().await?;
        let mut req = Map::new();
        req.insert("fromBlock".to_string(), Value::String("0x1".to_string()));
        req.insert("toBlock".to_string(), Value::String("0x8".to_string()));
        req.insert(
            "address".to_string(),
            serde_json::to_value(addresses.clone()).unwrap(),
        );
        join(
            client.call_method("eth_getLogs", Params::Array(vec![Value::Object(req)])),
            server,
        )
    };
    let logs = fut.await.0.unwrap();
    let logs = serde_json::from_value::<Vec<Log>>(logs).unwrap();
    assert_eq!(logs.len(), 36);
    for log in logs {
        assert!(addresses.contains(&log.address));
    }

    // Checks that topic filter works correctly
    let topics = vec![
        rpc_app
            .logs_helper
            .topic_by_event(Event::ERCTransfer)
            .unwrap(),
        rpc_app
            .logs_helper
            .topic_by_event(Event::ZkSyncChangePubKey)
            .unwrap(),
        rpc_app
            .logs_helper
            .topic_by_event(Event::ZkSyncDeposit)
            .unwrap(),
    ];

    let fut = {
        let (client, server) = local_client().await?;
        let mut req = Map::new();
        req.insert("fromBlock".to_string(), Value::String("0x1".to_string()));
        req.insert("toBlock".to_string(), Value::String("0x8".to_string()));
        req.insert("address".to_string(), Value::Null);
        req.insert(
            "topics".to_string(),
            Value::Array(vec![serde_json::to_value(topics.clone()).unwrap()]),
        );
        join(
            client.call_method("eth_getLogs", Params::Array(vec![Value::Object(req)])),
            server,
        )
    };
    let logs = fut.await.0.unwrap();
    let logs = serde_json::from_value::<Vec<Log>>(logs).unwrap();
    assert_eq!(logs.len(), 23);
    for log in logs {
        assert!(topics.contains(&log.topics[0]));
    }

    Ok(())
}
