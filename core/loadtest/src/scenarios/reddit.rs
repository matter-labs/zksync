//! Loadtest scenario for the Reddit PoC.
//!
//! This test runs the following operations:
//!
//! - 100,000 point claims (minting & distributing points) (i.e. transfers — AG)
//! - 25,000 subscriptions (i.e. creating subscriptions; this can be done fully offchain — AG)
//! - 75,000 one-off points burning (i.e. subscription redemptions: — AG)
//! - 100,000 transfers

// Scenario logic:
// - Create 25.000 users (via change pubkey op)
// - Execute 4 minting txs per user (total of 100.000)
// - Subscribe every user to the community (25.000 subscriptions)
// - Create 3 burning txs per user (75.000 burning txs)
// - Create 4 transfers per user (100.000 transfers)
// Additional: measure time to run the test.

// Built-in deps
use std::{
    iter::Iterator,
    sync::atomic::{AtomicU32, Ordering},
    time::Duration,
};
// External deps
use chrono::Utc;
use futures::future::try_join_all;
use num::BigUint;
use tokio::fs;
use web3::{
    transports::{EventLoopHandle, Http},
    types::H256,
};
// Workspace deps
use models::node::{closest_packable_fee_amount, tx::PackedEthSignature, FranklinTx, PrivateKey};
use testkit::zksync_account::ZksyncAccount;
// Local deps
use crate::{
    rpc_client::RpcClient,
    scenarios::{
        configs::RedditConfig,
        utils::{deposit_single, wait_for_commit},
        ScenarioContext,
    },
    test_accounts::TestAccount,
};

const N_ACCOUNTS: usize = 25_000;
const COMMUNITY_NAME: &str = "TestCommunity";

const REPORT_RATE: usize = 50; // How many iterations to execute quietly before reporting the execution state.

#[derive(Debug)]
struct ScenarioExecutor {
    rpc_client: RpcClient,

    /// Genesis account to mint tokens from.
    genesis_account: TestAccount,

    /// Burn account: account to which burned tokens are sent.
    burn_account: ZksyncAccount,

    /// ID and symbol of used token (e.g. `(0, "ETH")`).
    token: (u16, String),

    /// Intermediate accounts.
    accounts: Vec<ZksyncAccount>,

    /// Created subscription accounts.
    subscription_accounts: Vec<ZksyncAccount>,

    /// Amount of time to wait for one zkSync block to be verified.
    verify_timeout: Duration,

    /// Counter for operations.
    counter: AtomicU32,

    /// Event loop handle so transport for Eth account won't be invalidated.
    _event_loop_handle: EventLoopHandle,
}

impl ScenarioExecutor {
    /// Creates a real-life scenario executor.
    pub fn new(ctx: &ScenarioContext, rpc_client: RpcClient) -> Self {
        // Load the config for the test from JSON file.
        let config = RedditConfig::load(&ctx.config_path);

        // Create a transport for Ethereum account.
        let (_event_loop_handle, transport) =
            Http::new(&ctx.options.web3_url).expect("http transport start");

        // Create genesis account to mint tokens from.
        let genesis_account =
            TestAccount::from_info(&config.genesis_account, &transport, &ctx.options);

        // Create a burn account to burn tokens.
        // TODO: Burn account should be deterministic, not random.
        let burn_account = ZksyncAccount::rand();

        // Generate random accounts to rotate funds within.
        let (accounts, subscription_accounts) = (0..N_ACCOUNTS)
            .map(|_| {
                let account = ZksyncAccount::rand();
                let subscription_account =
                    Self::create_subscription_account(&account, COMMUNITY_NAME);

                (account, subscription_account)
            })
            .unzip();

        let verify_timeout = Duration::from_secs(config.block_timeout);

        Self {
            rpc_client,

            genesis_account,
            burn_account,
            accounts,
            subscription_accounts,
            verify_timeout,
            token: (config.token_id, config.token_name),

            counter: 0.into(),

            _event_loop_handle,
        }
    }

    /// Infallible test runner which performs the emergency exit if any step of the test
    /// fails.
    pub async fn run(&mut self) {
        if let Err(error) = self.run_test().await {
            log::error!("Loadtest erred with the following error: {}", error);
        } else {
            log::info!("Loadtest completed successfully");
        }
    }

    /// Method to be used before the scenario.
    /// It stores all the zkSync account keys into a file named
    /// like "reddit_accounts_2020_05_05_12_23_55.txt"
    /// so we can gain access to every account created in the loadtest.
    ///
    /// If saving the file fails, the accounts are printed to the log.
    async fn save_accounts(&self) {
        // Timestamp is used to generate unique file name postfix.
        let timestamp = Utc::now();
        let timestamp_str = timestamp.format("%Y_%m_%d_%H_%M_%S").to_string();

        let output_file_name = format!("reddit_accounts_{}.txt", timestamp_str);

        let mut account_list = String::new();

        // Add all the accounts to the string.
        // Debug representations of account contains both zkSync and Ethereum private keys.
        for account in self.accounts.iter() {
            account_list += &format!("{:?}\n", account);
        }

        // If we're unable to save the file, print its contents to the console at least.
        if let Err(error) = fs::write(&output_file_name, &account_list).await {
            log::error!(
                "Storing the account list erred with the following error: {}",
                error
            );
            log::warn!(
                "Printing the account list to the log instead: \n{}",
                account_list
            )
        } else {
            log::info!(
                "Accounts used in this test are saved to the file '{}'",
                &output_file_name
            );
        }
    }

    /// Runs the test step-by-step. Every test step is encapsulated into its own function.
    pub async fn run_test(&mut self) -> Result<(), failure::Error> {
        const PARALLEL_JOBS: usize = 50;

        self.save_accounts().await;

        self.initialize().await?;

        log::info!("Initialization complete, starting the test");

        for account_id in (0..N_ACCOUNTS).step_by(PARALLEL_JOBS) {
            let range_start = account_id;
            let range_end = account_id + PARALLEL_JOBS;
            let account_futures: Vec<_> = (range_start..range_end)
                .map(|account_id| self.one_account_run(account_id))
                .collect();

            try_join_all(account_futures).await?;
        }

        // TODO:
        // After executing these futures we must send one more (random) tx and wait it to be
        // verified. The verification will mean that all the previously sent txs are verified as well.
        // After that, we may check the balances of every account to check if all the txs were executed
        // successfully.

        self.finish().await?;

        Ok(())
    }

    /// Initializes the test, preparing the both main account and all the intermediate accounts for the interaction.
    async fn initialize(&mut self) -> Result<(), failure::Error> {
        if self.token.0 == 0 {
            log::info!("Token ID is 0 (ETH). Assuming that genesis account has to be initialized");

            // 1. Update the account nonce.
            self.genesis_account
                .update_nonce_values(&self.rpc_client)
                .await?;

            // 2. Perform a deposit
            deposit_single(
                &self.genesis_account,
                BigUint::from(1_000_000_000_000_000_000u64),
                &self.rpc_client,
            )
            .await?;

            // 3. Set the account ID.
            let resp = self
                .rpc_client
                .account_state_info(self.genesis_account.zk_acc.address)
                .await
                .expect("rpc error");
            assert!(resp.id.is_some(), "Account ID is none for new account");
            self.genesis_account.zk_acc.set_account_id(resp.id);

            // 4. Send the `ChangePubKey` tx to be able to work with account.
            let change_pubkey_tx = FranklinTx::ChangePubKey(Box::new(
                self.genesis_account
                    .zk_acc
                    .create_change_pubkey_tx(None, true, false),
            ));

            let tx_hash = self.rpc_client.send_tx(change_pubkey_tx, None).await?;

            wait_for_commit(tx_hash, Duration::from_secs(60), &self.rpc_client).await?;
            log::info!("Genesis account initialized successfully");
        } else {
            log::info!(
                "Token for test is not ETH (id: {}, symbol: {}). \
                Assuming that account was initialized externally.",
                self.token.0,
                &self.token.1
            );

            // We only have to update the account ID.
            let resp = self
                .rpc_client
                .account_state_info(self.genesis_account.zk_acc.address)
                .await
                .expect("rpc error");
            assert!(resp.id.is_some(), "Account ID is none for new account");
            self.genesis_account.zk_acc.set_account_id(resp.id);
        }

        // After the main wallet initialization, we have to initialize all the intermediate accounts
        // (both the main "user" accounts and subscription wallets).
        self.initialize_accounts().await?;

        Ok(())
    }

    async fn initialize_accounts(&self) -> Result<(), failure::Error> {
        log::info!("Initializing the Reddit accounts...");
        self.initialize_accounts_batch(&self.accounts).await?;
        log::info!("Initializing the subscription accounts...");
        self.initialize_accounts_batch(&self.subscription_accounts)
            .await?;
        Ok(())
    }

    async fn initialize_accounts_batch(
        &self,
        accounts: &[ZksyncAccount],
    ) -> Result<(), failure::Error> {
        // 1. Send the `Transfer` with 0 amount to each account to initialize it.
        let mut tx_hashes = Vec::new();
        for (account_id, account) in accounts.iter().enumerate() {
            let from_acc = &self.genesis_account.zk_acc;
            let to_acc = account;

            let fee = self.transfer_fee(&to_acc).await;
            let (transfer_tx, eth_sign) = self.sign_transfer(from_acc, to_acc, 0u64, fee);

            let tx_hash = self.rpc_client.send_tx(transfer_tx, eth_sign).await?;

            tx_hashes.push(tx_hash);

            let num_processed = account_id + 1;
            if num_processed % REPORT_RATE == 0 {
                log::info!(
                    "Sent {} / {} initial transfers...",
                    num_processed,
                    N_ACCOUNTS
                );
            }
        }

        log::info!("All the initial transfers for current batch are sent");

        for (account_id, tx_hash) in tx_hashes.into_iter().enumerate() {
            wait_for_commit(tx_hash, Duration::from_secs(60), &self.rpc_client).await?;

            let num_processed = account_id + 1;
            if num_processed % REPORT_RATE == 0 {
                log::info!(
                    "Committed {} / {} initial transfers...",
                    num_processed,
                    N_ACCOUNTS
                );
            }
        }

        log::info!("All the initial transfers for current batch are committed");

        // 2. Set the account ID and call the `ChangePubKey` to be able to work with account.
        let mut tx_hashes = Vec::new();
        for (account_id, account) in accounts.iter().enumerate() {
            let resp = self
                .rpc_client
                .account_state_info(account.address)
                .await
                .expect("rpc error");
            assert!(resp.id.is_some(), "Account ID is none for new account");
            account.set_account_id(resp.id);

            let change_pubkey_tx = FranklinTx::ChangePubKey(Box::new(
                account.create_change_pubkey_tx(None, true, false),
            ));

            let tx_hash = self.rpc_client.send_tx(change_pubkey_tx, None).await?;

            tx_hashes.push(tx_hash);

            let num_processed = account_id + 1;
            if num_processed % REPORT_RATE == 0 {
                log::info!(
                    "Sent {} / {} ChangePubKey transactions...",
                    num_processed,
                    N_ACCOUNTS
                );
            }
        }

        log::info!("All the ChangePubKey transactions for current batch are sent");

        for (account_id, tx_hash) in tx_hashes.into_iter().enumerate() {
            wait_for_commit(tx_hash, Duration::from_secs(60), &self.rpc_client).await?;

            let num_processed = account_id + 1;
            if num_processed % REPORT_RATE == 0 {
                log::info!(
                    "Committed {} / {} ChangePubKey transactions...",
                    num_processed,
                    N_ACCOUNTS
                );
            }
        }

        log::info!("All the ChangePubKey transactions for current batch are committed");

        Ok(())
    }

    async fn one_account_run(&self, account_id: usize) -> Result<(), failure::Error> {
        const N_MINT_OPS: usize = 4;
        const N_SUBSCRIPTIONS: usize = 1;
        const N_BURN_FUNDS_OPS: usize = 3;
        const N_TRANSFER_OPS: usize = 4;

        let account = &self.accounts[account_id];
        let subscription_wallet = &self.subscription_accounts[account_id];

        for _ in 0..N_MINT_OPS {
            self.mint_tokens(account).await?
        }

        for _ in 0..N_SUBSCRIPTIONS {
            self.subscribe(account, subscription_wallet).await?
        }

        for _ in 0..N_BURN_FUNDS_OPS {
            self.burn_funds(account).await?
        }

        for _ in 0..N_TRANSFER_OPS {
            self.transfer_funds(account).await?
        }

        self.counter.fetch_add(1, Ordering::SeqCst);
        let total = self.counter.load(Ordering::SeqCst);
        if total % REPORT_RATE as u32 == 0 {
            log::info!(
                "Performing loadtest... {} / {} iterations complete",
                total,
                N_ACCOUNTS
            );
        }

        Ok(())
    }

    async fn mint_tokens(&self, account: &ZksyncAccount) -> Result<(), failure::Error> {
        const MINT_SIZE: u64 = 100; // 100 tokens for everybody.

        // 1. Create a minting tx, signed by both participants.
        let from_acc = &self.genesis_account.zk_acc;
        let to_acc = account;

        let fee = self.transfer_fee(&to_acc).await;
        let mint_tx = self.sign_transfer_from(from_acc, to_acc, MINT_SIZE, fee);

        // 2. Send the tx.
        let tx_hash = self.rpc_client.send_tx(mint_tx, None).await?;

        // 3. Wait for it to be executed.
        wait_for_commit(tx_hash, Duration::from_secs(60), &self.rpc_client).await?;

        Ok(())
    }

    async fn subscribe(
        &self,
        account: &ZksyncAccount,
        subscription_wallet: &ZksyncAccount,
    ) -> Result<(), failure::Error> {
        const SUBSCRIPTION_COST: u64 = 1;

        // 1. Create a TransferFrom tx.
        let from_acc = account;
        let to_acc = &subscription_wallet;

        let fee = self.transfer_fee(&to_acc).await;
        let transfer_from_tx = self.sign_transfer_from(from_acc, to_acc, SUBSCRIPTION_COST, fee);

        // 2. Create a Burn tx
        let from_acc = &subscription_wallet;
        let to_acc = &self.burn_account;

        let fee = self.transfer_fee(&to_acc).await;
        let (burn_tx, burn_eth_sign) = self.sign_transfer(from_acc, to_acc, SUBSCRIPTION_COST, fee);

        // 3. Send both txs in a bundle.
        let txs = vec![(transfer_from_tx, None), (burn_tx, burn_eth_sign)];
        let tx_hashes = self.rpc_client.send_txs_batch(txs).await?;

        // 4. Wait for txs to be executed.
        for tx_hash in tx_hashes {
            wait_for_commit(tx_hash, Duration::from_secs(60), &self.rpc_client).await?;
        }

        Ok(())
    }

    async fn burn_funds(&self, account: &ZksyncAccount) -> Result<(), failure::Error> {
        const BURN_SIZE: u64 = 1; // Burn 1 token at a time.

        // 1. Create a minting tx, signed by both participants.
        let from_acc = account;
        let to_acc = &self.burn_account;

        let fee = self.transfer_fee(&to_acc).await;
        let (burn_tx, eth_sign) = self.sign_transfer(from_acc, to_acc, BURN_SIZE, fee);

        // 2. Send the tx.
        let tx_hash = self.rpc_client.send_tx(burn_tx, eth_sign).await?;

        // 3. Wait for it to be executed.
        wait_for_commit(tx_hash, Duration::from_secs(60), &self.rpc_client).await?;

        Ok(())
    }

    async fn transfer_funds(&self, account: &ZksyncAccount) -> Result<(), failure::Error> {
        const TRANSFER_SIZE: u64 = 1; // Send 1 token.

        // 1. Create a transfer tx (to self for simplicity).
        let from_acc = account;
        let to_acc = account;

        let fee = self.transfer_fee(account).await;
        let (tx, eth_sign) = self.sign_transfer(from_acc, to_acc, TRANSFER_SIZE, fee);

        // 2. Send the tx.
        let tx_hash = self
            .rpc_client
            .send_tx(tx.clone(), eth_sign.clone())
            .await?;

        // 3. Wait for it to be executed.
        wait_for_commit(tx_hash, Duration::from_secs(60), &self.rpc_client).await?;

        Ok(())
    }

    async fn finish(&mut self) -> Result<(), failure::Error> {
        Ok(())
    }

    /// Obtains a fee required for the transfer operation.
    async fn transfer_fee(&self, to_acc: &ZksyncAccount) -> BigUint {
        let fee = self
            .rpc_client
            .get_tx_fee("Transfer", to_acc.address, &self.token.1)
            .await
            .expect("Can't get tx fee");

        closest_packable_fee_amount(&fee)
    }

    /// Creates a signed transfer transaction.
    /// Sender and receiver are chosen from the generated
    /// accounts, determined by its indices.
    fn sign_transfer(
        &self,
        from: &ZksyncAccount,
        to: &ZksyncAccount,
        amount: impl Into<BigUint>,
        fee: impl Into<BigUint>,
    ) -> (FranklinTx, Option<PackedEthSignature>) {
        let (tx, eth_signature) = from.sign_transfer(
            self.token.0,
            &self.token.1,
            amount.into(),
            fee.into(),
            &to.address,
            None,
            true,
        );

        (FranklinTx::Transfer(Box::new(tx)), Some(eth_signature))
    }

    /// Creates a signed TransferFrom transaction. Transaction will be signed by both participants of
    /// the transfer.
    /// Ethereum signature is not required for this operation
    fn sign_transfer_from(
        &self,
        from: &ZksyncAccount,
        to: &ZksyncAccount,
        amount: impl Into<BigUint>,
        fee: impl Into<BigUint>,
    ) -> FranklinTx {
        let tx = to.sign_transfer_from(from, self.token.0, amount.into(), fee.into(), None, true);

        FranklinTx::TransferFrom(Box::new(tx))
    }

    fn create_subscription_account(account: &ZksyncAccount, community_name: &str) -> ZksyncAccount {
        let mut sk_bytes = [0u8; 32];
        account
            .private_key
            .write(&mut sk_bytes[..])
            .expect("Can't write the private key");
        let seed = format!("{}reddit.com/r/{}", hex::encode(&sk_bytes), community_name);
        let private_key_bytes = private_key_from_seed(seed.as_ref());

        let zk_private_key =
            PrivateKey::read(&private_key_bytes[..]).expect("Can't read private key [zk]");
        let eth_private_key = H256::from_slice(&private_key_bytes[..]);

        let address = PackedEthSignature::address_from_private_key(&eth_private_key)
            .expect("Can't get the address from private key");

        ZksyncAccount::new(zk_private_key, Default::default(), address, eth_private_key)
    }
}

/// Deterministic algorithm to generate a private key for subscription.
/// This implementation is copied from the `zksync-crypto` crate to completely
/// match the function used in the js on the client side.
fn private_key_from_seed(seed: &[u8]) -> Vec<u8> {
    pub use crypto_exports::franklin_crypto::bellman::pairing::bn256::{Bn256 as Engine, Fr};
    use crypto_exports::franklin_crypto::{
        alt_babyjubjub::fs::FsRepr,
        bellman::pairing::ff::{PrimeField, PrimeFieldRepr},
        jubjub::JubjubEngine,
    };
    use sha2::{Digest, Sha256};
    pub type Fs = <Engine as JubjubEngine>::Fs;

    fn sha256_bytes(input: &[u8]) -> Vec<u8> {
        let mut hasher = Sha256::new();
        hasher.input(input);
        hasher.result().to_vec()
    };

    if seed.len() < 32 {
        panic!("Seed is too short");
    };

    let mut effective_seed = sha256_bytes(seed);

    loop {
        let raw_priv_key = sha256_bytes(&effective_seed);
        let mut fs_repr = FsRepr::default();
        fs_repr
            .read_le(&raw_priv_key[..])
            .expect("failed to read raw_priv_key");
        if Fs::from_repr(fs_repr).is_ok() {
            return raw_priv_key;
        } else {
            effective_seed = raw_priv_key;
        }
    }
}

/// Runs the real-life test scenario.
/// For description, see the module doc-comment.
pub fn run_scenario(mut ctx: ScenarioContext) {
    let rpc_addr = ctx.rpc_addr.clone();
    let rpc_client = RpcClient::new(&rpc_addr);

    let mut scenario = ScenarioExecutor::new(&ctx, rpc_client);

    // Run the scenario.
    log::info!("Starting the real-life test");
    ctx.rt.block_on(scenario.run());
}
