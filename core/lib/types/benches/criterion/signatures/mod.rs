use criterion::{black_box, criterion_group, BatchSize, Bencher, Criterion, Throughput};
use zksync_basic_types::H256;
use zksync_crypto::franklin_crypto::eddsa::PrivateKey;
use zksync_crypto::rand::{Rng, SeedableRng, XorShiftRng};
use zksync_types::tx::{PackedEthSignature, TxSignature};

fn bench_signature_zksync_musig_verify(b: &mut Bencher<'_>) {
    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);
    const WITHDRAW_TX_LEN: usize = 65;

    let pk = PrivateKey(rng.gen());
    let message = rng
        .gen_iter::<u8>()
        .take(WITHDRAW_TX_LEN)
        .collect::<Vec<_>>();

    let setup = || (TxSignature::sign_musig(&pk, &message), message.clone());

    b.iter_batched(
        setup,
        |(signature, msg)| {
            black_box(signature.verify_musig(&msg));
        },
        BatchSize::SmallInput,
    );
}

fn bench_signature_verify_eth_packed(b: &mut Bencher<'_>) {
    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);
    const TYPICAL_ETH_SIGNATURE_LEN: usize = 150;

    let pk = H256(rng.gen());

    let message = rng
        .gen_iter::<u8>()
        .take(TYPICAL_ETH_SIGNATURE_LEN)
        .collect::<Vec<_>>();

    let signature = PackedEthSignature::sign(&pk, &message).unwrap();

    let setup = || (signature.clone(), message.clone());

    b.iter_batched(
        setup,
        |(signature, msg)| {
            let _ = black_box(signature.signature_recover_signer(&msg));
        },
        BatchSize::SmallInput,
    );
}

/// For reference, raw speed of optimized signature library
fn bench_signature_seckp_recover(b: &mut Bencher<'_>) {
    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);

    let message = secp256k1::Message::from_slice(&rng.gen::<[u8; 32]>()).expect("msg creation");
    let secret_key =
        &secp256k1::SecretKey::from_slice(&rng.gen::<[u8; 32]>()).expect("secret key creation");

    let secp = secp256k1::Secp256k1::new();
    let signature = secp.sign_recoverable(&message, &secret_key);

    let verify_secp = secp256k1::Secp256k1::verification_only();

    let setup = || (&verify_secp, message, signature);
    b.iter_batched(
        setup,
        |(secp, msg, sign)| {
            let _ = black_box(secp.recover(&msg, &sign));
        },
        BatchSize::SmallInput,
    );
}

pub fn bench_signatures(c: &mut Criterion) {
    let mut group = c.benchmark_group("Signature verify");
    group.throughput(Throughput::Elements(1));
    group.bench_function(
        "bench_signature_verify_zksync_musig",
        bench_signature_zksync_musig_verify,
    );
    group.bench_function(
        "bench_signature_verify_eth_packed",
        bench_signature_verify_eth_packed,
    );
    group.bench_function(
        "bench_signature_seckp_recover",
        bench_signature_seckp_recover,
    );
    group.finish();
}

criterion_group!(signature_benches, bench_signatures);
