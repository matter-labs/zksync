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

// Temporary, for development

#![allow(dead_code)]

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
enum TestPhase {
    Init,
    Deposit,
    InitialTransfer,
    FundsRotation,
    CollectingFunds,
    Withdraw,
    Finish,
}

#[derive(Debug)]
struct ScenarioExecutor {
    phase: TestPhase,
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
            phase: TestPhase::Init,
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
        self.deposit().await?;
        self.initial_transfer().await?;
        self.funds_rotation().await?;
        self.collect_funds().await?;
        self.withdraw().await?;
        self.finish().await?;

        Ok(())
    }

    async fn deposit(&mut self) -> Result<(), failure::Error> {
        self.phase = TestPhase::Deposit;

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

        log::info!("Deposit phase completed");

        Ok(())
    }

    /// Creates a signed transfer to new transaction.
    /// Sender is the main account, receiver is one of the generated
    /// accounts, determined by its index.
    fn sign_transfer_to_new(
        &self,
        to_idx: usize,
        amount: u64,
    ) -> (FranklinTx, Option<PackedEthSignature>) {
        let to = &self.accounts[to_idx].address;

        let (tx, eth_signature) = self.main_account.zk_acc.sign_transfer(
            0, // ETH
            "ETH",
            BigDecimal::from(amount),
            BigDecimal::from(0),
            &to,
            None,
            true,
        );

        (FranklinTx::Transfer(Box::new(tx)), Some(eth_signature))
    }

    async fn initial_transfer(&mut self) -> Result<(), failure::Error> {
        self.phase = TestPhase::InitialTransfer;

        log::info!(
            "Starting initial transfer. {} wei will be send to each of {} new accounts",
            self.transfer_size,
            self.n_accounts
        );

        let signed_transfers: Vec<_> = (0..self.n_accounts)
            .map(|acc_id| self.sign_transfer_to_new(acc_id, self.transfer_size))
            .collect();

        log::info!("Signed all the initial transfer transactions, sending");

        // Send txs by batches that can fit in one block.
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

            // Wait until all the transactions are verified.
            wait_for_verify(sent_txs, TIMEOUT_FOR_BLOCK, &self.rpc_client).await;
        }

        log::info!("Sent all the initial transfer transactions");

        Ok(())
    }

    async fn funds_rotation(&mut self) -> Result<(), failure::Error> {
        self.phase = TestPhase::FundsRotation;

        Ok(())
    }

    async fn collect_funds(&mut self) -> Result<(), failure::Error> {
        self.phase = TestPhase::CollectingFunds;

        Ok(())
    }

    async fn withdraw(&mut self) -> Result<(), failure::Error> {
        self.phase = TestPhase::Withdraw;

        Ok(())
    }

    async fn finish(&mut self) -> Result<(), failure::Error> {
        self.phase = TestPhase::Finish;

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
