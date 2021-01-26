use async_jsonrpc_client::{Params, SubscriptionId, Transport, WebSocketTransport};
use criterion::async_executor::{AsyncExecutor, FuturesExecutor};
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ethabi::Address;
use serde_json::{json, Map, Value};

async fn websocket_client() {
    let ws_path = std::env::var("API_JSON_RPC_WS_URL").unwrap();
    let ws = WebSocketTransport::new(ws_path);
    let address = Address::random();
    // Filecoin.Version need read permission
    let params = json!({"tx_types": vec!["Withdraw", "Withdraw"], "addresses": vec![address, address], "token": "WBTC"});
    if let Value::Object(obj) = params {
        let version: Value = ws
            .send("get_txs_batch_fee_in_wei", Params::Map(obj))
            .await
            .unwrap();
        println!("Version: {:?}", version);
    }
}

fn from_elem(c: &mut Criterion) {
    let size: usize = 1024;
    c.bench_with_input(BenchmarkId::new("input_example", size), &size, |b, &s| {
        // Insert a call to `to_async` to convert the bencher to async mode.
        // The timing loops are the same as with the normal bencher.
        b.to_async(FuturesExecutor).iter(|| websocket_client());
    });
}

criterion_group!(benches, from_elem);
criterion_main!(benches);
