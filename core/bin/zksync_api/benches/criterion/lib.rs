use criterion::{criterion_group, criterion_main, Criterion};
use ethabi::Address;
use reqwest::{blocking::Client, StatusCode};

use zksync_api_client::rest::v1::{IncomingTxBatchForFee, IncomingTxForFee};
use zksync_types::{TokenLike, TxFeeTypes};

fn generate_transactions(number: usize) -> IncomingTxBatchForFee {
    let mut tx_types = vec![];
    let mut addresses = vec![];
    for _ in 0..number {
        tx_types.push(TxFeeTypes::Withdraw);
        addresses.push(Address::random());
    }
    IncomingTxBatchForFee {
        tx_types,
        addresses,
        token_like: TokenLike::Symbol("wBTC".to_string()),
    }
}

fn get_txs_batch_fee(client: Client, url: String, transaction: IncomingTxBatchForFee) {
    let res = client
        .post(format!("{}/api/v1/transactions/fee/batch", url).as_str())
        .json(&transaction)
        .send()
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK)
}

fn get_txs_fee(client: Client, url: String) {
    let transaction = IncomingTxForFee {
        tx_type: TxFeeTypes::Withdraw,
        address: Address::random(),
        token_like: TokenLike::Symbol("wBTC".to_string()),
    };

    let res = client
        .post(format!("{}/api/v1/transactions/fee", url).as_str())
        .json(&transaction)
        .send()
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK)
}

fn bench_fee(c: &mut Criterion) {
    let url = std::env::var("API_REST_URL").unwrap();
    let client = reqwest::blocking::Client::new();
    let transaction = generate_transactions(100);
    c.bench_function("get_txs_batch_fee_new_version", |b| {
        b.iter(|| get_txs_batch_fee(client.clone(), url.clone(), transaction.clone()))
    });
    c.bench_function("get_txs_fee", |b| {
        b.iter(|| get_txs_fee(client.clone(), url.clone()))
    });
}

criterion_group!(benches, bench_fee);
criterion_main!(benches);
