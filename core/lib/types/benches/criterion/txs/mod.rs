//! Benchmarks for the zkSync transactions.

// Built-in deps
// External uses
use criterion::{black_box, criterion_group, BatchSize, Criterion, Throughput};
use num::BigUint;
use web3::types::H256;
// Workspace uses
use zksync_crypto::{
    params::MIN_NFT_TOKEN_ID,
    priv_key_from_fs,
    rand::{thread_rng, Rng},
    PrivateKey,
};
use zksync_types::{
    account::{Account, PubKeyHash},
    tx::{
        ChangePubKey, ForcedExit, MintNFT, Order, PackedEthSignature, Swap, Transfer, Withdraw,
        WithdrawNFT,
    },
    AccountId, Address, Nonce, TokenId,
};
// Local uses
use zksync_types::tx::{ChangePubKeyECDSAData, ChangePubKeyEthAuthData};

const ETH_TOKEN_ID: TokenId = TokenId(0x00);

/// Creates a random ZKSync account.
fn generate_account() -> (H256, PrivateKey, Account) {
    let default_balance = 1_000_000u32.into();

    let rng = &mut thread_rng();
    let sk = priv_key_from_fs(rng.gen());

    let eth_sk = H256::random();
    let address = PackedEthSignature::address_from_private_key(&eth_sk)
        .expect("Can't get address from the ETH secret key");

    let mut account = Account::default_with_address(&address);
    account.pub_key_hash = PubKeyHash::from_privkey(&sk);
    account.set_balance(ETH_TOKEN_ID, default_balance);

    (eth_sk, sk, account)
}

pub struct TxBenchSetup {
    eth_pk: H256,
    private_key: PrivateKey,
    account: Account,
}

impl TxBenchSetup {
    pub fn new() -> Self {
        let (eth_pk, private_key, account) = generate_account();
        Self {
            eth_pk,
            private_key,
            account,
        }
    }

    pub fn create_transfer(&self, with_cache: bool) -> Transfer {
        let mut tx = Transfer::new_signed(
            AccountId(0),
            self.account.address,
            Address::random(),
            ETH_TOKEN_ID,
            10u32.into(),
            1u32.into(),
            Nonce(0),
            Default::default(),
            &self.private_key,
        )
        .expect("Failed to sign Transfer");

        if !with_cache {
            tx.wipe_signer_cache();
        }

        tx
    }

    pub fn create_withdraw(&self, with_cache: bool) -> Withdraw {
        let mut tx = Withdraw::new_signed(
            AccountId(0),
            self.account.address,
            Address::random(),
            ETH_TOKEN_ID,
            10u32.into(),
            1u32.into(),
            Nonce(0),
            Default::default(),
            &self.private_key,
        )
        .expect("Failed to sign Withdraw");

        if !with_cache {
            tx.wipe_signer_cache();
        }

        tx
    }

    pub fn create_change_pubkey(&self, with_cache: bool) -> ChangePubKey {
        let rng = &mut thread_rng();
        let new_sk = priv_key_from_fs(rng.gen());

        let mut tx = ChangePubKey::new(
            AccountId(0),
            self.account.address,
            PubKeyHash::from_privkey(&new_sk),
            ETH_TOKEN_ID,
            Default::default(),
            Nonce(0),
            Default::default(),
            None,
            None,
            None,
        );

        tx.eth_auth_data = {
            let sign_bytes = tx
                .get_eth_signed_data()
                .expect("Failed to construct ChangePubKey signed message.");
            let eth_signature =
                PackedEthSignature::sign(&self.eth_pk, &sign_bytes).expect("Signing failed");
            Some(ChangePubKeyEthAuthData::ECDSA(ChangePubKeyECDSAData {
                eth_signature,
                batch_hash: H256::zero(),
            }))
        };

        // We signed transaction manually, so it doesn't have a signature cache by default.
        // So unlike in other functions, we call `check_correctness` if needed to create such a cache.
        if with_cache {
            let _ = tx.check_correctness();
        }

        tx
    }

    pub fn create_forced_exit(&self, with_cache: bool) -> ForcedExit {
        let mut tx = ForcedExit::new_signed(
            AccountId(0),
            self.account.address,
            ETH_TOKEN_ID,
            Default::default(),
            Nonce(0),
            Default::default(),
            &self.private_key,
        )
        .expect("Failed to sign ForcedExit");

        if !with_cache {
            tx.wipe_signer_cache();
        }

        tx
    }

    pub fn create_mint_nft(&self, with_cache: bool) -> MintNFT {
        let mut tx = MintNFT::new_signed(
            AccountId(0),
            self.account.address,
            H256::random(),
            Address::random(),
            Default::default(),
            ETH_TOKEN_ID,
            Nonce(0),
            &self.private_key,
        )
        .expect("Failed to sign MintNFT");

        if !with_cache {
            tx.wipe_signer_cache();
        }

        tx
    }

    pub fn create_withdraw_nft(&self, with_cache: bool) -> WithdrawNFT {
        let mut tx = WithdrawNFT::new_signed(
            AccountId(0),
            self.account.address,
            self.account.address,
            TokenId(MIN_NFT_TOKEN_ID),
            ETH_TOKEN_ID,
            Default::default(),
            Nonce(0),
            Default::default(),
            &self.private_key,
        )
        .expect("Failed to sign WithdrawNFT");

        if !with_cache {
            tx.wipe_signer_cache();
        }

        tx
    }

    pub fn create_swap(&self, with_cache: bool) -> Swap {
        let (_, acc_0_sk, acc_0) = generate_account();
        let (_, acc_1_sk, acc_1) = generate_account();
        let token_0 = TokenId(0);
        let token_1 = TokenId(1);

        let order_0 = Order::new_signed(
            AccountId(0),
            acc_0.address,
            Nonce(0),
            token_0,
            token_1,
            (BigUint::from(1u64), BigUint::from(1u64)),
            1u64.into(),
            Default::default(),
            &acc_0_sk,
        )
        .expect("order creation failed");

        let order_1 = Order::new_signed(
            AccountId(1),
            acc_1.address,
            Nonce(0),
            token_1,
            token_0,
            (BigUint::from(1u64), BigUint::from(1u64)),
            1u64.into(),
            Default::default(),
            &acc_1_sk,
        )
        .expect("order creation failed");

        let mut tx = Swap::new_signed(
            AccountId(2),
            self.account.address,
            Nonce(0),
            (order_0, order_1),
            (BigUint::from(1u64), BigUint::from(1u64)),
            Default::default(),
            ETH_TOKEN_ID,
            &self.private_key,
        )
        .expect("swap creation failed");

        if !with_cache {
            tx.wipe_signer_cache();
        }

        tx
    }
}

pub fn bench_txs(c: &mut Criterion) {
    const INPUT_SIZE: Throughput = Throughput::Elements(1);

    // Verify signature benches (cached / uncached).
    for with_cache in [false, true] {
        let cache = if with_cache {
            "(cached)"
        } else {
            "(not cached)"
        };

        let group_name = format!("Verify signature {}", cache);
        let mut group = c.benchmark_group(&group_name);
        // Setup the input size so the throughput will be reported.
        group.throughput(INPUT_SIZE);

        group.bench_function("Transfer::verify_signature", |b| {
            let tx = TxBenchSetup::new().create_transfer(with_cache);
            b.iter(|| black_box(tx.verify_signature()));
        });
        group.bench_function("Withdraw::verify_signature", |b| {
            let tx = TxBenchSetup::new().create_withdraw(with_cache);
            b.iter(|| black_box(tx.verify_signature()));
        });
        group.bench_function("ChangePubKey::verify_signature", |b| {
            let tx = TxBenchSetup::new().create_change_pubkey(with_cache);
            b.iter(|| black_box(tx.verify_signature()));
        });
        group.bench_function("ForcedExit::verify_signature", |b| {
            let tx = TxBenchSetup::new().create_forced_exit(with_cache);
            b.iter(|| black_box(tx.verify_signature()));
        });
        group.bench_function("MintNFT::verify_signature", |b| {
            let tx = TxBenchSetup::new().create_mint_nft(with_cache);
            b.iter(|| black_box(tx.verify_signature()));
        });
        group.bench_function("WithdrawNFT::verify_signature", |b| {
            let tx = TxBenchSetup::new().create_withdraw_nft(with_cache);
            b.iter(|| black_box(tx.verify_signature()));
        });
        group.bench_function("Swap::verify_signature", |b| {
            let tx = TxBenchSetup::new().create_swap(with_cache);
            b.iter(|| black_box(tx.verify_signature()));
        });

        group.finish();
    }

    // Check correctness benches (cached / uncached).
    for with_cache in [false, true] {
        let cache = if with_cache {
            "(cached)"
        } else {
            "(not cached)"
        };

        let group_name = format!("Check correctness {}", cache);
        let mut group = c.benchmark_group(&group_name);
        // Setup the input size so the throughput will be reported.
        group.throughput(INPUT_SIZE);

        group.bench_function("Transfer::check_correctness", |b| {
            let tx = TxBenchSetup::new().create_transfer(with_cache);
            b.iter_batched(
                || tx.clone(),
                |mut tx| black_box(tx.check_correctness()),
                BatchSize::SmallInput,
            );
        });
        group.bench_function("Withdraw::check_correctness", |b| {
            let tx = TxBenchSetup::new().create_withdraw(with_cache);
            b.iter_batched(
                || tx.clone(),
                |mut tx| black_box(tx.check_correctness()),
                BatchSize::SmallInput,
            );
        });
        group.bench_function("ChangePubKey::check_correctness", |b| {
            let tx = TxBenchSetup::new().create_change_pubkey(with_cache);
            b.iter_batched(
                || tx.clone(),
                |mut tx| black_box(tx.check_correctness()),
                BatchSize::SmallInput,
            );
        });
        group.bench_function("ForcedExit::check_correctness", |b| {
            let tx = TxBenchSetup::new().create_forced_exit(with_cache);
            b.iter_batched(
                || tx.clone(),
                |mut tx| black_box(tx.check_correctness()),
                BatchSize::SmallInput,
            );
        });
        group.bench_function("MintNFT::check_correctness", |b| {
            let tx = TxBenchSetup::new().create_mint_nft(with_cache);
            b.iter_batched(
                || tx.clone(),
                |mut tx| black_box(tx.check_correctness()),
                BatchSize::SmallInput,
            );
        });
        group.bench_function("WithdrawNFT::check_correctness", |b| {
            let tx = TxBenchSetup::new().create_withdraw_nft(with_cache);
            b.iter_batched(
                || tx.clone(),
                |mut tx| black_box(tx.check_correctness()),
                BatchSize::SmallInput,
            );
        });
        group.bench_function("Swap::check_correctness", |b| {
            let tx = TxBenchSetup::new().create_swap(with_cache);
            b.iter_batched(
                || tx.clone(),
                |mut tx| black_box(tx.check_correctness()),
                BatchSize::SmallInput,
            );
        });

        group.finish();
    }
}

criterion_group!(txs_benches, bench_txs);
