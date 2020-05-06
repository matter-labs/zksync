//! Real-life loadtest scenario does not measure the TPS nor simulated the high load,
//! but rather simulates the real-life use case of zkSync:
//!
//! 1. Funds are deposited from one Ethereum account into one new zkSync account.
//! 2. Once funds are deposited, this account split the funds between N accounts
//!    using the `transferToNew` operation.
//! 3. Once funds are transferred and verified, these funds are "rotated" within
//!    created accounts using the `transfer` operation. This operation is repeated
//!    M times.
//! 4. To finish the test, all the funds are collected back to the initial account
//!    are withdrawn to the Ethereum.
//!
//! `N` and `M` are configurable parameters, meaning the breadth of the test (how
//! many accounts will be used within the test) and the depth of the test (how
//! many rotation cycles are performed) correspondingly.
//!
//! Schematically, scenario will look like this:
//!
//! Deposit  | Transfer to new  | Transfer | Collect back | Withdraw to ETH
//!
//! ```text
//!                                ┗━━━━┓
//!                      ┏━━━>Acc1━━━━━┓┗>Acc1━━━┓
//!                    ┏━┻━━━>Acc2━━━━┓┗━>Acc2━━━┻┓
//! ETH━━━━>InitialAcc━╋━━━━━>Acc3━━━┓┗━━>Acc3━━━━╋━>InitialAcc━>ETH
//!                    ┗━┳━━━>Acc4━━┓┗━━━>Acc4━━━┳┛
//!                      ┗━━━>Acc5━┓┗━━━━>Acc5━━━┛
//! ```

// Built-in deps
use std::time::Duration;
// External deps
use bigdecimal::BigDecimal;
// Workspace deps
use models::{
    config_options::ConfigurationOptions,
    node::{tx::PackedEthSignature, FranklinTx},
};
use testkit::zksync_account::ZksyncAccount;
// Local deps
use crate::{
    rpc_client::RpcClient,
    scenarios::{
        utils::{deposit_single, wait_for_verify},
        ScenarioContext,
    },
    sent_transactions::SentTransactions,
    test_accounts::TestAccount,
};

/// Transactions in this test are aligned so that we aren't sending more transactions
/// than could fit in the block at the time.
/// So the timeout is set to the value reasonable for one block.
const TIMEOUT_FOR_BLOCK: Duration = Duration::from_secs(2 * 60);

#[derive(Debug)]
struct ScenarioExecutor {
    rpc_client: RpcClient,

    /// Main account to deposit ETH from / return ETH back to.
    main_account: TestAccount,

    /// Intermediate account to rotate funds within.
    accounts: Vec<ZksyncAccount>,

    /// Amount of intermediate accounts.
    n_accounts: usize,
    /// Transfer amount per accounts (in wei).
    transfer_size: u64,
    /// Amount of cycles for funds rotation.
    cycles_amount: usize,

    /// Biggest supported block size (to not overload the node
    /// with too many txs at the moment)
    max_block_size: usize,
}

impl ScenarioExecutor {
    pub fn new(main_account: TestAccount, rpc_client: RpcClient) -> Self {
        // Temporary constants to be replaced with configurable values.
        const N_ACCOUNTS: usize = 100;
        const TRANSFER_SIZE: u64 = 100;
        const CYCLES_AMOUNT: usize = 10;

        let accounts = (0..N_ACCOUNTS).map(|_| ZksyncAccount::rand()).collect();

        Self {
            rpc_client,

            main_account,
            accounts,

            n_accounts: N_ACCOUNTS,
            transfer_size: TRANSFER_SIZE,
            cycles_amount: CYCLES_AMOUNT,
            max_block_size: Self::get_max_supported_block_size(),
        }
    }

    pub async fn run(&mut self) -> Result<(), failure::Error> {
        self.start().await?;
        self.deposit().await?;
        self.initial_transfer().await?;
        self.funds_rotation().await?;
        self.collect_funds().await?;
        self.withdraw().await?;
        self.finish().await?;

        Ok(())
    }

    async fn start(&mut self) -> Result<(), failure::Error> {
        // First of all, we have to update both the Ethereum and ZKSync accounts nonce values.
        self.main_account
            .update_nonce_values(&self.rpc_client)
            .await?;

        Ok(())
    }

    async fn deposit(&mut self) -> Result<(), failure::Error> {
        // Amount of money we need to deposit.
        // Initialize it with the raw amount: only sum of transfers per account.
        // Fees will be set to zero, so there is no need in any additional funds.
        let amount_to_deposit = self.transfer_size * self.n_accounts as u64;

        log::info!(
            "Starting depositing phase. Depositing {} wei to the main account",
            amount_to_deposit
        );

        // Deposit funds and wait for operation to be executed.
        deposit_single(
            &self.main_account,
            amount_to_deposit.into(),
            &self.rpc_client,
        )
        .await?;

        log::info!("Deposit sent and verified");

        // Now when deposits are done it is time to update account id.
        self.main_account
            .update_account_id(&self.rpc_client)
            .await?;

        log::info!("Main account ID set");

        // ...and change the main account pubkey.
        // We have to change pubkey after the deposit so we'll be able to use corresponding
        // `zkSync` account.
        let (change_pubkey_tx, eth_sign) = (self.main_account.sign_change_pubkey(), None);
        let mut sent_txs = SentTransactions::new();
        let tx_hash = self.rpc_client.send_tx(change_pubkey_tx, eth_sign).await?;
        sent_txs.add_tx_hash(tx_hash);
        wait_for_verify(sent_txs, TIMEOUT_FOR_BLOCK, &self.rpc_client).await;

        log::info!("Main account pubkey changed");

        log::info!("Deposit phase completed");

        Ok(())
    }

    /// Creates a signed transfer transaction.
    /// Sender and receiver are chosen from the generated
    /// accounts, determined by its indices.
    fn sign_transfer(
        &self,
        from: &ZksyncAccount,
        to: &ZksyncAccount,
        amount: u64,
    ) -> (FranklinTx, Option<PackedEthSignature>) {
        let (tx, eth_signature) = from.sign_transfer(
            0, // ETH
            "ETH",
            BigDecimal::from(amount),
            BigDecimal::from(0),
            &to.address,
            None,
            true,
        );

        (FranklinTx::Transfer(Box::new(tx)), Some(eth_signature))
    }

    /// Generates an ID for funds transfer. The ID is the ID of the next
    /// account, treating the accounts array like a circle buffer:
    /// given 3 accounts, IDs returned for queries (0, 1, 2) will be
    /// (1, 2, 0) correspondingly.
    fn acc_for_transfer(&self, from_idx: usize) -> usize {
        (from_idx + 1) % self.accounts.len()
    }

    async fn initial_transfer(&mut self) -> Result<(), failure::Error> {
        log::info!(
            "Starting initial transfer. {} wei will be send to each of {} new accounts",
            self.transfer_size,
            self.n_accounts
        );

        let signed_transfers: Vec<_> = (0..self.n_accounts)
            .map(|to_idx| {
                let from_acc = &self.main_account.zk_acc;
                let to_acc = &self.accounts[to_idx];
                self.sign_transfer(from_acc, to_acc, self.transfer_size)
            })
            .collect();

        log::info!("Signed all the initial transfer transactions, sending");

        // Send txs by batches that can fit in one block.
        let to_verify = signed_transfers.len();
        let mut verified = 0;
        for tx_batch in signed_transfers.chunks(self.max_block_size) {
            let mut sent_txs = SentTransactions::new();
            // Send each tx.
            for (tx, eth_sign) in tx_batch {
                let tx_hash = self
                    .rpc_client
                    .send_tx(tx.clone(), eth_sign.clone())
                    .await?;
                sent_txs.add_tx_hash(tx_hash);
            }

            verified += sent_txs.len();

            // Wait until all the transactions are verified.
            wait_for_verify(sent_txs, TIMEOUT_FOR_BLOCK, &self.rpc_client).await;

            log::info!("Sent and verified {}/{} txs", verified, to_verify);
        }

        log::info!("All the initial transfers are completed");
        log::info!("Updating the accounts info and changing their public keys");

        // After all the initial transfer completed, we have to update new account IDs
        // and change public keys of accounts (so we'll be able to send transfers from them).
        let mut sent_txs = SentTransactions::new();
        for account in self.accounts.iter() {
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
            sent_txs.add_tx_hash(tx_hash);
        }
        // Calculate the estimated amount of blocks for all the txs to be processed.
        let n_blocks = (self.accounts.len() / self.max_block_size + 1) as u32;
        wait_for_verify(sent_txs, TIMEOUT_FOR_BLOCK * n_blocks, &self.rpc_client).await;

        log::info!("All the accounts are prepared");

        log::info!("Initial transfers are sent and verified");

        Ok(())
    }

    async fn funds_rotation(&mut self) -> Result<(), failure::Error> {
        for step_number in 1..=self.cycles_amount {
            log::info!("Starting funds rotation cycle {}", step_number);

            self.funds_rotation_step().await?;
        }

        Ok(())
    }

    async fn funds_rotation_step(&mut self) -> Result<(), failure::Error> {
        let signed_transfers: Vec<_> = (0..self.n_accounts)
            .map(|from_id| {
                let from_acc = &self.accounts[from_id];
                let to_id = self.acc_for_transfer(from_id);
                let to_acc = &self.accounts[to_id];
                self.sign_transfer(from_acc, to_acc, self.transfer_size)
            })
            .collect();

        log::info!("Signed transfers, sending");

        // Send txs by batches that can fit in one block.
        let to_verify = signed_transfers.len();
        let mut verified = 0;
        for tx_batch in signed_transfers.chunks(self.max_block_size) {
            let mut sent_txs = SentTransactions::new();
            // Send each tx.
            for (tx, eth_sign) in tx_batch {
                let tx_hash = self
                    .rpc_client
                    .send_tx(tx.clone(), eth_sign.clone())
                    .await?;
                sent_txs.add_tx_hash(tx_hash);
            }

            verified += sent_txs.len();

            // Wait until all the transactions are verified.
            wait_for_verify(sent_txs, TIMEOUT_FOR_BLOCK, &self.rpc_client).await;

            log::info!("Sent and verified {}/{} txs", verified, to_verify);
        }

        log::info!("Transfers are sent and verified");

        Ok(())
    }

    async fn collect_funds(&mut self) -> Result<(), failure::Error> {
        log::info!("Starting collecting funds back to the main account",);

        let signed_transfers: Vec<_> = (0..self.n_accounts)
            .map(|from_id| {
                let from_acc = &self.accounts[from_id];
                let to_acc = &self.main_account.zk_acc;
                self.sign_transfer(from_acc, to_acc, self.transfer_size)
            })
            .collect();

        log::info!("Signed transfers, sending");

        // Send txs by batches that can fit in one block.
        let to_verify = signed_transfers.len();
        let mut verified = 0;
        for tx_batch in signed_transfers.chunks(self.max_block_size) {
            let mut sent_txs = SentTransactions::new();
            // Send each tx.
            for (tx, eth_sign) in tx_batch {
                let tx_hash = self
                    .rpc_client
                    .send_tx(tx.clone(), eth_sign.clone())
                    .await?;
                sent_txs.add_tx_hash(tx_hash);
            }

            verified += sent_txs.len();

            // Wait until all the transactions are verified.
            wait_for_verify(sent_txs, TIMEOUT_FOR_BLOCK, &self.rpc_client).await;

            log::info!("Sent and verified {}/{} txs", verified, to_verify);
        }

        log::info!("Collecting funds completed");
        Ok(())
    }

    async fn withdraw(&mut self) -> Result<(), failure::Error> {
        log::info!("Withdrawing funds back to the Ethereum");

        let mut sent_txs = SentTransactions::new();

        let amount_to_withdraw = self.transfer_size * self.n_accounts as u64;

        let (tx, eth_sign) = self
            .main_account
            .sign_withdraw_single(amount_to_withdraw.into());
        let tx_hash = self
            .rpc_client
            .send_tx(tx.clone(), eth_sign.clone())
            .await?;
        sent_txs.add_tx_hash(tx_hash);

        wait_for_verify(sent_txs, TIMEOUT_FOR_BLOCK, &self.rpc_client).await;

        log::info!("Withdrawing funds completed");

        Ok(())
    }

    async fn finish(&mut self) -> Result<(), failure::Error> {
        Ok(())
    }

    /// Loads the biggest supported block size.
    /// This method assumes that loadtest and server share the same env config,
    /// since the value is loaded from the env.
    fn get_max_supported_block_size() -> usize {
        let options = ConfigurationOptions::from_env();

        *options.available_block_chunk_sizes.iter().max().unwrap()
    }
}

/// Runs the outgoing TPS scenario:
/// sends the different types of transactions, and measures the TPS for the sending
/// process (in other words, speed of the ZKSync node mempool).
pub fn run_scenario(mut ctx: ScenarioContext) {
    // let verify_timeout_sec = Duration::from_secs(ctx.ctx.verify_timeout_sec);
    let rpc_addr = ctx.rpc_addr.clone();

    let rpc_client = RpcClient::new(&rpc_addr);

    let mut test_accounts = ctx.test_accounts;

    let mut scenario = ScenarioExecutor::new(test_accounts.pop().unwrap(), rpc_client);

    // Obtain the Ethereum node JSON RPC address.
    log::info!("Starting the loadtest");

    // Run the scenario.
    log::info!("Waiting for all transactions to be verified");
    ctx.rt
        .block_on(scenario.run())
        .expect("Failed the scenario");
    log::info!("Loadtest completed.");
}
