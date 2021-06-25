use criterion::{criterion_group, criterion_main, Criterion};
use ethabi::Address;
use reqwest::{blocking::Client, StatusCode};

use zksync_api_types::v02::fee::{
    ApiTxFeeTypes, BatchFeeRequest, TxFeeRequest, TxInBatchFeeRequest,
};
use zksync_types::{TokenId, TokenLike};

fn generate_transactions(number: usize) -> BatchFeeRequest {
    let mut transactions = Vec::new();
    for _ in 0..number {
        transactions.push(TxInBatchFeeRequest {
            tx_type: ApiTxFeeTypes::Withdraw,
            address: Address::random(),
        });
    }
    BatchFeeRequest {
        transactions,
        token_like: TokenLike::Id(TokenId(2)), // id of wBTC on localhost
    }
}

fn get_txs_batch_fee(client: Client, url: String, batch_fee_request: BatchFeeRequest) {
    let res = client
        .post(format!("{}/api/v0.2/fee/batch", url).as_str())
        .json(&batch_fee_request)
        .send()
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK)
}

fn get_txs_fee(client: Client, url: String) {
    let transaction = TxFeeRequest {
        tx_type: ApiTxFeeTypes::Withdraw,
        address: Address::random(),
        token_like: TokenLike::Id(TokenId(2)), // id of wBTC on localhost
    };

    let res = client
        .post(format!("{}/api/v0.2/fee", url).as_str())
        .json(&transaction)
        .send()
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK)
}

fn bench_fee(c: &mut Criterion) {
    let url = std::env::var("API_REST_URL").unwrap();
    let client = reqwest::blocking::Client::new();
    let transaction = generate_transactions(100);
    c.bench_function("get_txs_batch_fee", |b| {
        b.iter(|| get_txs_batch_fee(client.clone(), url.clone(), transaction.clone()))
    });
    c.bench_function("get_txs_fee", |b| {
        b.iter(|| get_txs_fee(client.clone(), url.clone()))
    });
}

criterion_group!(benches, bench_fee);
criterion_main!(benches);
