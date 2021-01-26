use async_jsonrpc_client::{Params, SubscriptionId, Transport, WebSocketTransport};
use criterion::async_executor::{AsyncExecutor, FuturesExecutor};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ethabi::Address;
use reqwest::StatusCode;
use serde_json::{json, Map, Value};
use zksync_api_client::rest::v1::IncomingTxBatchForFee;
use zksync_types::{Token, TokenLike, TxFeeTypes};

fn get_txs_batch_fee() {
    let url = std::env::var("API_REST_URL").unwrap();
    let client = reqwest::blocking::Client::new();

    let res = client
        .post(format!("{}/api/v1/transactions/batch_fee", url).as_str())
        .json(&IncomingTxBatchForFee {
            tx_types: vec![TxFeeTypes::Withdraw],
            addresses: vec![Address::random()],
            token_like: TokenLike::Symbol("wBTC".to_string()),
        })
        .send()
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK)
}

fn bench_fee(c: &mut Criterion) {
    c.bench_function("get_txs_batch_fee", get_txs_batch_fee());
}

criterion_group!(benches, bench_fee);
criterion_main!(benches);
