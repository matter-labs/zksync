//! Gas price test is used to calculate costs of user transactions in terms of gas price.
//! It should be used as a fast benchmark tool for optimizations of out smart contracts, and
//! as a sanity check after contract refactorings.
//!
//! It is important for several reasons:
//! * Transfer cost determines maximum possible TPS of our network in larbe block size limit.
//! * Cost of operations in the verify functions could stop block verification because of the block gas limit.
//! * It is useful to calculate cost of the "griefing" attack.
//! We don't take fees for deposit and full exit, but we must process them, so it is possible to spam us and force us to spend money.

use crate::eth_account::EthereumAccount;
use crate::external_commands::{deploy_contracts, get_test_accounts};
use crate::zksync_account::ZkSyncAccount;
use num::{rational::Ratio, traits::Pow, BigInt, BigUint};
use std::ops::Mul;
use std::str::FromStr;
use web3::transports::Http;
use web3::types::{H256, U256};
use zksync_crypto::params::{
    AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH, FEE_EXPONENT_BIT_WIDTH,
    FEE_MANTISSA_BIT_WIDTH,
};
use zksync_crypto::rand::{Rng, SeedableRng, XorShiftRng};
use zksync_crypto::{priv_key_from_fs, rand};
use zksync_testkit::zksync_account::ZkSyncETHAccountData;
use zksync_testkit::*;
use zksync_types::{
    helpers::{pack_fee_amount, pack_token_amount, unpack_fee_amount, unpack_token_amount},
    tx::ChangePubKeyCREATE2Data,
    Address, ChangePubKeyOp, DepositOp, FullExitOp, MintNFTOp, Nonce, PubKeyHash, SwapOp, TokenId,
    TransferOp, TransferToNewOp, WithdrawNFTOp, WithdrawOp,
};
use zksync_utils::UnsignedRatioSerializeAsDecimal;

const MIN_BLOCK_SIZE_CHUNKS: usize = 0;

/// Gas cost data from one test in one test we process `samples` number of operations in one block.
#[derive(Debug, Clone)]
struct CostsSample {
    /// number of operations in the test
    samples: usize,
    /// Total gas that user spent in this test
    users_gas_cost: BigInt,
    /// Operator commit gas cost
    commit_cost: BigInt,
    /// Operator verify gas cost
    verify_cost: BigInt,
    /// Operator withdrawal gas cost
    withdrawals_cost: BigInt,
}

impl CostsSample {
    pub fn new(samples: usize, users_gas_cost: U256, block_result: BlockExecutionResult) -> Self {
        Self {
            samples,
            users_gas_cost: u256_to_bigint(users_gas_cost),
            commit_cost: block_result
                .commit_result
                .gas_used
                .map(u256_to_bigint)
                .expect("commit gas used"),
            verify_cost: block_result
                .verify_result
                .gas_used
                .map(u256_to_bigint)
                .expect("verify gas used"),
            withdrawals_cost: block_result
                .withdrawals_result
                .gas_used
                .map(u256_to_bigint)
                .expect("withdrawals gas used"),
        }
    }

    fn sub_per_operation(&self, base_cost: &BaseCost) -> CostPerOperation {
        let samples = self.samples;

        let user_gas_cost = &self.users_gas_cost / samples;

        let commit_cost = (&self.commit_cost - &base_cost.base_commit_cost) / samples;
        let verify_cost = (&self.verify_cost - &base_cost.base_verify_cost) / samples;
        let withdraw_cost = (&self.withdrawals_cost - &base_cost.base_withdraw_cost) / samples;
        let total = &commit_cost + &verify_cost + &withdraw_cost;

        CostPerOperation {
            user_gas_cost,
            commit_cost,
            verify_cost,
            withdraw_cost,
            total,
        }
    }

    pub fn report(&self, base_cost: &BaseCost, description: &str, report_grief: bool) {
        let per_operation_cost = self.sub_per_operation(base_cost);
        per_operation_cost.report(description, report_grief);
    }
}

/// User gas cost of performing one operation and additional gas cost
/// that operator spends in each of the processing step.
///
/// # Note
///
/// * Operation cost can be negative, because some operations reclaims storage slots.
/// * Operation gas cost for some operations (e.g. Deposit) depends on sample size
#[derive(Debug, Clone)]
struct CostPerOperation {
    user_gas_cost: BigInt,
    commit_cost: BigInt,
    verify_cost: BigInt,
    withdraw_cost: BigInt,
    total: BigInt,
}

impl CostPerOperation {
    /// Grief factor when we neglect base commit/verify cost (when blocks are big)
    fn asymptotic_grief_factor(&self) -> String {
        let operator_total_cost_per_op = &self.commit_cost + &self.verify_cost;
        UnsignedRatioSerializeAsDecimal::serialize_to_str_with_dot(
            &Ratio::new(
                self.user_gas_cost
                    .to_biguint()
                    .expect("user gas cost is negative"),
                operator_total_cost_per_op
                    .to_biguint()
                    .expect("operator total cost is negative"),
            ),
            4,
        )
    }

    pub fn report(&self, description: &str, report_grief: bool) {
        let grief_info = if report_grief {
            let mut info = String::from("\nuser gas cost over operator cost: ");
            info.push_str(&self.asymptotic_grief_factor());
            info
        } else {
            String::new()
        };
        println!(
            "Gas cost of {}:\nuser_gas_cost: {}\ncommit: {}\nprove: {}\nexecute: {}\ntotal: {}{}",
            description,
            self.user_gas_cost,
            self.commit_cost,
            self.verify_cost,
            self.withdraw_cost,
            self.total,
            grief_info
        );
        println!()
    }
}

/// Base cost of commit of one operation, we determine it by executing empty block. (with 2 noops)
#[derive(Debug, Clone)]
struct BaseCost {
    base_commit_cost: BigInt,
    base_verify_cost: BigInt,
    base_withdraw_cost: BigInt,
}

fn u256_to_bigint(u256: U256) -> BigInt {
    BigInt::from_str(&u256.to_string()).unwrap()
}

fn gen_packable_amount(rng: &mut impl Rng) -> BigUint {
    let mantissa =
        BigUint::from(rng.gen_range(0u64, 2u64.pow(AMOUNT_MANTISSA_BIT_WIDTH as u32) - 1));
    let exponent = BigUint::from(10u32).pow(2u32.pow(AMOUNT_EXPONENT_BIT_WIDTH as u32) - 1);
    let truncated_amount = (mantissa * exponent) % BigUint::from(2u128.pow(5 * 8));

    unpack_token_amount(&pack_token_amount(&truncated_amount)).expect("Failed to repack amount")
}

fn gen_packable_fee(rng: &mut impl Rng) -> BigUint {
    let mantissa = BigUint::from(rng.gen_range(0u64, 2u64.pow(FEE_MANTISSA_BIT_WIDTH as u32) - 1));
    let exponent = BigUint::from(10u32).pow(2u32.pow(FEE_EXPONENT_BIT_WIDTH as u32) - 1);
    let truncated_fee = (mantissa * exponent) % BigUint::from(2u128.pow(5 * 8));
    unpack_fee_amount(&pack_fee_amount(&truncated_fee)).expect("Failed to repack fee")
}

fn gen_unpacked_amount(rng: &mut impl Rng) -> BigUint {
    BigUint::from(rng.gen_range(0u64, 2u64.pow(5 * 4)))
        * BigUint::from(rng.gen_range(0u64, 2u64.pow(5 * 4)))
}

async fn gas_price_test() {
    let testkit_config = TestkitConfig::from_env();

    let fee_account = ZkSyncAccount::rand();
    let (sk_thread_handle, stop_state_keeper_sender, sk_channels) =
        spawn_state_keeper(&fee_account.address, genesis_state(&fee_account.address));

    let genesis_root = genesis_state(&fee_account.address).tree.root_hash();

    let contracts = deploy_contracts(false, genesis_root);

    let transport = Http::new(&testkit_config.web3_url).expect("http transport start");
    let (test_accounts_info, commit_account_info) = get_test_accounts();
    let commit_account = EthereumAccount::new(
        commit_account_info.private_key,
        commit_account_info.address,
        transport.clone(),
        contracts.contract,
        testkit_config.chain_id,
        testkit_config.gas_price_factor,
    );
    let eth_accounts = test_accounts_info
        .into_iter()
        .map(|test_eth_account| {
            EthereumAccount::new(
                test_eth_account.private_key,
                test_eth_account.address,
                transport.clone(),
                contracts.contract,
                testkit_config.chain_id,
                testkit_config.gas_price_factor,
            )
        })
        .collect::<Vec<_>>();

    let zksync_accounts = {
        let mut zksync_accounts = vec![fee_account];
        zksync_accounts.extend(eth_accounts.iter().map(|eth_account| {
            let rng_zksync_key = ZkSyncAccount::rand().private_key;
            ZkSyncAccount::new(
                rng_zksync_key,
                Nonce(0),
                eth_account.address,
                ZkSyncETHAccountData::EOA {
                    eth_private_key: eth_account.private_key,
                },
            )
        }));
        zksync_accounts
    };

    let accounts = AccountSet {
        eth_accounts,
        zksync_accounts,
        fee_account_id: ZKSyncAccountId(0),
    };

    let mut test_setup = TestSetup::new(
        sk_channels,
        accounts,
        &contracts,
        commit_account,
        genesis_root,
        None,
    );

    let rng = &mut XorShiftRng::from_seed([0, 1, 2, 3]);

    commit_cost_of_n_empty_blocks(&mut test_setup, 1).await; // warmup, init some storage slots
    let base_cost = commit_cost_of_n_empty_blocks(&mut test_setup, 1).await;
    {
        // Aggregated blocks amortization info
        let n_blocks = 5;
        let base_cost_n_blocks = commit_cost_of_n_empty_blocks(&mut test_setup, n_blocks).await;
        let commit_cost_per_block = (base_cost_n_blocks.base_commit_cost
            - base_cost.base_commit_cost.clone())
            / (n_blocks - 1);
        let commit_base_cost = &base_cost.base_commit_cost - &commit_cost_per_block;
        let prove_cost_per_block = (base_cost_n_blocks.base_verify_cost
            - base_cost.base_verify_cost.clone())
            / (n_blocks - 1);
        let prove_base_cost = &base_cost.base_verify_cost - &prove_cost_per_block;
        let execute_cost_per_block = (base_cost_n_blocks.base_withdraw_cost
            - base_cost.base_withdraw_cost.clone())
            / (n_blocks - 1);
        let execute_base_cost = &base_cost.base_withdraw_cost - &execute_cost_per_block;
        println!("Cost of block operations (base_cost, cost_per_block):");
        println!("NOTE: aggregated blocks(n) cost of tx = base_cost + cost_per_block*n");
        println!(
            "commit: ({}, {})\nprove: ({}, {})\nexecute: ({}, {})",
            commit_base_cost,
            commit_cost_per_block,
            prove_base_cost,
            prove_cost_per_block,
            execute_base_cost,
            execute_cost_per_block
        );
        println!();
    }

    commit_cost_of_deposits(&mut test_setup, 100, Token(TokenId(0)), rng)
        .await
        .report(&base_cost, "deposit ETH", true);
    commit_cost_of_deposits(&mut test_setup, 50, Token(TokenId(1)), rng)
        .await
        .report(&base_cost, "deposit ERC20", true);

    commit_cost_of_create2_change_pubkey(&mut test_setup, 50)
        .await
        .report(&base_cost, "create2 change pubkey", false);
    commit_cost_of_onchain_change_pubkey(&mut test_setup, 50)
        .await
        .report(&base_cost, "onchain change pubkey", false);
    commit_cost_of_change_pubkey(&mut test_setup, 50)
        .await
        .report(&base_cost, "change pubkey", false);

    commit_cost_of_transfers(&mut test_setup, 500, rng)
        .await
        .report(&base_cost, "transfer", false);
    commit_cost_of_transfers_to_new(&mut test_setup, 500, rng)
        .await
        .report(&base_cost, "transfer to new", false);
    commit_cost_of_swaps(&mut test_setup, 60, rng)
        .await
        .report(&base_cost, "swap", false);
    commit_cost_of_mint_nft(&mut test_setup, 60, rng)
        .await
        .report(&base_cost, "mint nft", false);

    commit_cost_of_full_exits(&mut test_setup, 100, Token(TokenId(0)))
        .await
        .report(&base_cost, "full exit ETH", true);
    commit_cost_of_full_exits(&mut test_setup, 100, Token(TokenId(1)))
        .await
        .report(&base_cost, "full exit ERC20", true);

    commit_cost_of_withdrawals(&mut test_setup, 40, Token(TokenId(0)), rng)
        .await
        .report(&base_cost, "withdrawals ETH", false);
    commit_cost_of_withdrawals(&mut test_setup, 40, Token(TokenId(1)), rng)
        .await
        .report(&base_cost, "withdrawals ERC20", false);
    commit_cost_of_withdrawals_nft(&mut test_setup, 10, rng)
        .await
        .report(&base_cost, "withdrawals NFT", false);

    stop_state_keeper_sender.send(()).expect("sk stop send");
    sk_thread_handle.join().expect("sk thread join");
}

async fn commit_cost_of_mint_nft(
    test_setup: &mut TestSetup,
    n_mint_nfts: usize,
    rng: &mut impl Rng,
) -> CostsSample {
    let mut tranfers_fee = Vec::new();
    let mut deposit_amount = BigUint::from(0u32);

    let mut content_hashes = Vec::new();
    let change_pk_fee = gen_packable_fee(rng);
    deposit_amount += &change_pk_fee;

    for _ in 0..n_mint_nfts {
        let amount = gen_packable_amount(rng);
        let fee = gen_packable_fee(rng);
        content_hashes.push(H256::random());
        deposit_amount += &amount + &fee;
        tranfers_fee.push(fee);
    }

    // Prepare block with transfers
    test_setup.start_block();
    test_setup
        .deposit(
            ETHAccountId(1),
            ZKSyncAccountId(1),
            Token(TokenId(0)),
            deposit_amount,
        )
        .await;
    // create account 2
    test_setup
        .deposit(
            ETHAccountId(2),
            ZKSyncAccountId(2),
            Token(TokenId(0)),
            BigUint::from(0u32),
        )
        .await;
    test_setup
        .change_pubkey_with_tx(ZKSyncAccountId(1), Token(TokenId(0)), 0u32.into())
        .await;
    test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");

    test_setup.start_block();
    for i in 0..n_mint_nfts {
        test_setup
            .mint_nft(
                ZKSyncAccountId(1),
                ZKSyncAccountId(2),
                Token(TokenId(0)),
                content_hashes[i],
                tranfers_fee[i].clone(),
            )
            .await;
    }
    let execute_result = test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");
    assert_eq!(
        execute_result.block_size_chunks,
        n_mint_nfts * MintNFTOp::CHUNKS,
        "block size mismatch"
    );
    execute_result.commit_result.gas_used.unwrap();
    CostsSample::new(n_mint_nfts, U256::from(0), execute_result)
}

async fn commit_cost_of_transfers(
    test_setup: &mut TestSetup,
    n_transfers: usize,
    rng: &mut impl Rng,
) -> CostsSample {
    let mut tranfers_amount = Vec::new();
    let mut tranfers_fee = Vec::new();
    let mut deposit_amount = BigUint::from(0u32);

    let change_pk_fee = gen_packable_fee(rng);
    deposit_amount += &change_pk_fee;

    for _ in 0..n_transfers {
        let amount = gen_packable_amount(rng);
        let fee = gen_packable_fee(rng);
        deposit_amount += &amount + &fee;
        tranfers_amount.push(amount);
        tranfers_fee.push(fee);
    }

    // Prepare block with transfers
    test_setup.start_block();
    test_setup
        .deposit(
            ETHAccountId(1),
            ZKSyncAccountId(1),
            Token(TokenId(0)),
            deposit_amount,
        )
        .await;
    // create account 2
    test_setup
        .deposit(
            ETHAccountId(2),
            ZKSyncAccountId(2),
            Token(TokenId(0)),
            BigUint::from(0u32),
        )
        .await;
    test_setup
        .change_pubkey_with_tx(ZKSyncAccountId(1), Token(TokenId(0)), 0u32.into())
        .await;
    test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");

    // Execute transfers
    test_setup.start_block();
    for i in 0..n_transfers {
        test_setup
            .transfer(
                ZKSyncAccountId(1),
                ZKSyncAccountId(2),
                Token(TokenId(0)),
                tranfers_amount[i].clone(),
                tranfers_fee[i].clone(),
                Default::default(),
            )
            .await;
    }
    let transfer_execute_result = test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");
    assert_eq!(
        transfer_execute_result.block_size_chunks,
        n_transfers * TransferOp::CHUNKS,
        "block size mismatch"
    );
    transfer_execute_result.commit_result.gas_used.unwrap();
    CostsSample::new(n_transfers, U256::from(0), transfer_execute_result)
}

async fn commit_cost_of_swaps(
    test_setup: &mut TestSetup,
    n_swaps: usize,
    rng: &mut impl Rng,
) -> CostsSample {
    let mut swap_amounts = Vec::new();
    let mut swap_fees = Vec::new();
    let change_pk_fee = gen_packable_fee(rng);
    let mut deposit_amount_0 = change_pk_fee.clone();
    let mut deposit_amount_1 = change_pk_fee.clone();
    let mut fee_amount = change_pk_fee.clone();

    for _ in 0..n_swaps {
        let amount_0 = gen_packable_amount(rng);
        let amount_1 = gen_packable_amount(rng);
        let fee = gen_packable_fee(rng);
        deposit_amount_0 += &amount_0;
        deposit_amount_1 += &amount_1;
        fee_amount += &fee;
        swap_amounts.push((amount_0, amount_1));
        swap_fees.push(fee);
    }

    // Prepare block with swaps
    test_setup.start_block();
    test_setup
        .deposit(
            ETHAccountId(1),
            ZKSyncAccountId(1),
            Token(TokenId(0)),
            deposit_amount_0,
        )
        .await;
    test_setup.start_block();
    test_setup
        .deposit(
            ETHAccountId(2),
            ZKSyncAccountId(2),
            Token(TokenId(1)),
            deposit_amount_1,
        )
        .await;
    test_setup
        .deposit(
            ETHAccountId(3),
            ZKSyncAccountId(3),
            Token(TokenId(1)),
            fee_amount,
        )
        .await;
    test_setup
        .deposit(
            ETHAccountId(1),
            ZKSyncAccountId(4),
            Token(TokenId(0)),
            0u32.into(),
        )
        .await;
    test_setup
        .deposit(
            ETHAccountId(1),
            ZKSyncAccountId(5),
            Token(TokenId(0)),
            0u32.into(),
        )
        .await;
    test_setup
        .change_pubkey_with_tx(ZKSyncAccountId(1), Token(TokenId(0)), 0u32.into())
        .await;
    test_setup
        .change_pubkey_with_tx(ZKSyncAccountId(2), Token(TokenId(1)), 0u32.into())
        .await;
    test_setup
        .change_pubkey_with_tx(ZKSyncAccountId(3), Token(TokenId(1)), 0u32.into())
        .await;
    test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");

    // Execute swaps
    test_setup.start_block();
    for i in 0..n_swaps {
        test_setup
            .swap(
                (ZKSyncAccountId(1), ZKSyncAccountId(2)),
                (ZKSyncAccountId(4), ZKSyncAccountId(5)),
                ZKSyncAccountId(3),
                (Token(TokenId(0)), Token(TokenId(1)), Token(TokenId(1))),
                swap_amounts[i].clone(),
                swap_fees[i].clone(),
                Default::default(),
            )
            .await;
    }
    let swap_execute_result = test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");
    assert_eq!(
        swap_execute_result.block_size_chunks,
        n_swaps * SwapOp::CHUNKS,
        "block size mismatch"
    );
    swap_execute_result.commit_result.gas_used.unwrap();
    CostsSample::new(n_swaps, U256::from(0), swap_execute_result)
}

async fn commit_cost_of_transfers_to_new(
    test_setup: &mut TestSetup,
    n_transfers: usize,
    rng: &mut impl Rng,
) -> CostsSample {
    let mut tranfers_amount = Vec::new();
    let mut tranfers_fee = Vec::new();
    let mut deposit_amount = BigUint::from(0u32);

    let change_pk_fee = gen_packable_fee(rng);
    deposit_amount += &change_pk_fee;

    for _ in 0..n_transfers {
        let amount = gen_packable_amount(rng);
        let fee = gen_packable_fee(rng);
        deposit_amount += &amount + &fee;
        tranfers_amount.push(amount);
        tranfers_fee.push(fee);
    }

    test_setup.start_block();
    test_setup
        .deposit(
            ETHAccountId(1),
            ZKSyncAccountId(1),
            Token(TokenId(0)),
            deposit_amount,
        )
        .await;
    test_setup
        .change_pubkey_with_tx(ZKSyncAccountId(1), Token(TokenId(0)), 0u32.into())
        .await;
    test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");

    test_setup.start_block();
    for i in 0..n_transfers {
        test_setup
            .transfer_to_new_random(
                ZKSyncAccountId(1),
                Token(TokenId(0)),
                tranfers_amount[i].clone(),
                tranfers_fee[i].clone(),
                rng,
            )
            .await;
    }
    let transfer_execute_result = test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");
    assert_eq!(
        transfer_execute_result.block_size_chunks,
        n_transfers * TransferToNewOp::CHUNKS,
        "block size mismatch"
    );
    CostsSample::new(n_transfers, U256::from(0), transfer_execute_result)
}

async fn commit_cost_of_withdrawals_nft(
    test_setup: &mut TestSetup,
    n_withdrawals: usize,
    rng: &mut impl Rng,
) -> CostsSample {
    let mut content_hashes = Vec::new();
    let mut withdrawals_fee = Vec::new();
    let mut deposit_amount = BigUint::from(0u32);

    let change_pk_fee = gen_packable_fee(rng);
    deposit_amount += &change_pk_fee;

    for _ in 0..n_withdrawals {
        let fee = gen_packable_fee(rng);
        deposit_amount += &fee.clone().mul(2u32);
        content_hashes.push(H256::random());
        withdrawals_fee.push(fee);
    }

    test_setup.start_block();
    test_setup
        .deposit(
            ETHAccountId(1),
            ZKSyncAccountId(1),
            Token(TokenId(0)),
            deposit_amount.clone(),
        )
        .await;
    test_setup
        .deposit(
            ETHAccountId(2),
            ZKSyncAccountId(2),
            Token(TokenId(0)),
            deposit_amount,
        )
        .await;
    test_setup
        .change_pubkey_with_tx(ZKSyncAccountId(1), Token(TokenId(0)), 0u32.into())
        .await;
    test_setup
        .change_pubkey_with_tx(ZKSyncAccountId(2), Token(TokenId(0)), 0u32.into())
        .await;
    let mut current_nft = test_setup.get_last_committed_nft_id().await;

    for i in 0..n_withdrawals {
        test_setup
            .mint_nft(
                ZKSyncAccountId(1),
                ZKSyncAccountId(2),
                Token(TokenId(0)),
                content_hashes[i],
                withdrawals_fee[i].clone(),
            )
            .await;
    }
    test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");

    test_setup.start_block();
    for fee in withdrawals_fee {
        current_nft += 1;
        test_setup
            .withdraw_nft(
                ZKSyncAccountId(2),
                Token(TokenId(current_nft)),
                Token(TokenId(0)),
                fee,
                rng,
            )
            .await;
    }

    let withdraws_execute_result = test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");
    assert_eq!(
        withdraws_execute_result.block_size_chunks,
        n_withdrawals * WithdrawNFTOp::CHUNKS,
        "block size mismatch"
    );
    CostsSample::new(n_withdrawals, U256::from(0), withdraws_execute_result)
}

async fn commit_cost_of_withdrawals(
    test_setup: &mut TestSetup,
    n_withdrawals: usize,
    token: Token,
    rng: &mut impl Rng,
) -> CostsSample {
    let mut withdraws_fee = Vec::new();
    let mut withdrawals_fee = Vec::new();
    let mut deposit_amount = BigUint::from(0u32);

    let change_pk_fee = gen_packable_fee(rng);
    deposit_amount += &change_pk_fee;

    for _ in 0..n_withdrawals {
        let amount = gen_unpacked_amount(rng);
        let fee = gen_packable_fee(rng);
        deposit_amount += &amount + &fee;
        withdraws_fee.push(amount);
        withdrawals_fee.push(fee);
    }

    test_setup.start_block();
    test_setup
        .deposit(ETHAccountId(1), ZKSyncAccountId(1), token, deposit_amount)
        .await;
    test_setup
        .change_pubkey_with_tx(ZKSyncAccountId(1), token, 0u32.into())
        .await;
    test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");

    test_setup.start_block();
    for i in 0..n_withdrawals {
        test_setup
            .withdraw_to_random_account(
                ZKSyncAccountId(1),
                token,
                withdraws_fee[i].clone(),
                withdrawals_fee[i].clone(),
                rng,
            )
            .await;
    }
    let withdraws_execute_result = test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");
    assert_eq!(
        withdraws_execute_result.block_size_chunks,
        n_withdrawals * WithdrawOp::CHUNKS,
        "block size mismatch"
    );
    CostsSample::new(n_withdrawals, U256::from(0), withdraws_execute_result)
}

async fn commit_cost_of_deposits(
    test_setup: &mut TestSetup,
    n_deposits: usize,
    token: Token,
    rng: &mut impl Rng,
) -> CostsSample {
    let mut amounts = Vec::new();
    for _ in 0..n_deposits {
        amounts.push(gen_unpacked_amount(rng));
    }

    let mut user_gas_cost = U256::from(0);
    test_setup.start_block();
    for amount in amounts.into_iter() {
        let deposit_tx_receipt = test_setup
            .deposit_to_random(ETHAccountId(4), token, amount.clone(), rng)
            .await
            .last()
            .cloned()
            .expect("At least one receipt is expected for deposit");
        user_gas_cost += deposit_tx_receipt.gas_used.expect("deposit gas used");
    }
    let deposits_execute_result = test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");
    assert_eq!(
        deposits_execute_result.block_size_chunks,
        n_deposits * DepositOp::CHUNKS,
        "block size mismatch"
    );
    CostsSample::new(n_deposits, user_gas_cost, deposits_execute_result)
}

async fn commit_cost_of_full_exits(
    test_setup: &mut TestSetup,
    n_full_exits: usize,
    token: Token,
) -> CostsSample {
    let mut user_gas_cost = U256::from(0);

    test_setup.start_block();
    test_setup
        .deposit(
            ETHAccountId(3),
            ZKSyncAccountId(4),
            token,
            BigUint::from(1u32),
        )
        .await;
    test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");

    test_setup.start_block();
    for _ in 0..n_full_exits {
        let (full_exit_tx_receipt, _) = test_setup
            .full_exit(ETHAccountId(3), ZKSyncAccountId(4), token)
            .await;
        user_gas_cost += full_exit_tx_receipt.gas_used.expect("full exit gas used");
    }
    let full_exits_execute_results = test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");
    assert_eq!(
        full_exits_execute_results.block_size_chunks,
        n_full_exits * FullExitOp::CHUNKS,
        "block size mismatch"
    );
    CostsSample::new(n_full_exits, user_gas_cost, full_exits_execute_results)
}

async fn commit_cost_of_n_empty_blocks(test_setup: &mut TestSetup, n: usize) -> BaseCost {
    let mut blocks = Vec::new();
    for _ in 0..n {
        test_setup.start_block();
        let block = test_setup.execute_block().await;
        assert_eq!(
            block.block_chunks_size, MIN_BLOCK_SIZE_CHUNKS,
            "block size mismatch"
        );
        blocks.push(block);
    }
    let base_commit_cost = test_setup
        .commit_blocks(&blocks)
        .await
        .expect_success()
        .gas_used
        .map(u256_to_bigint)
        .expect("commit gas used empty");
    let base_verify_cost = test_setup
        .prove_blocks(&blocks, None)
        .await
        .expect_success()
        .gas_used
        .map(u256_to_bigint)
        .expect("prove gas used empty");
    let base_withdraw_cost = test_setup
        .execute_blocks_onchain(&blocks)
        .await
        .expect_success()
        .gas_used
        .map(u256_to_bigint)
        .expect("execute gas used empty");
    BaseCost {
        base_commit_cost,
        base_verify_cost,
        base_withdraw_cost,
    }
}

async fn commit_cost_of_change_pubkey(
    test_setup: &mut TestSetup,
    n_change_pubkeys: usize,
) -> CostsSample {
    let token = Token(TokenId(0));
    let fee_amount = 100u32;
    let deposit_amount = (fee_amount * (n_change_pubkeys + 1) as u32).into();

    test_setup.start_block();
    test_setup
        .deposit(ETHAccountId(1), ZKSyncAccountId(1), token, deposit_amount)
        .await;
    test_setup
        .change_pubkey_with_tx(ZKSyncAccountId(1), token, 0u32.into())
        .await;
    test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");

    test_setup.start_block();
    for _ in 0..n_change_pubkeys {
        test_setup
            .change_pubkey_with_tx(ZKSyncAccountId(1), token, 0u32.into())
            .await;
    }
    let change_pubkey_execute_result = test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");
    assert_eq!(
        change_pubkey_execute_result.block_size_chunks,
        n_change_pubkeys * ChangePubKeyOp::CHUNKS,
        "block size mismatch"
    );
    CostsSample::new(
        n_change_pubkeys,
        U256::from(0),
        change_pubkey_execute_result,
    )
}

async fn commit_cost_of_onchain_change_pubkey(
    test_setup: &mut TestSetup,
    n_change_pubkeys: usize,
) -> CostsSample {
    let token = Token(TokenId(0));
    let fee_amount = 100u32;
    let deposit_amount = (fee_amount * (n_change_pubkeys + 1) as u32).into();

    test_setup.start_block();
    test_setup
        .deposit(ETHAccountId(1), ZKSyncAccountId(1), token, deposit_amount)
        .await;
    test_setup
        .change_pubkey_with_tx(ZKSyncAccountId(1), token, 0u32.into())
        .await;
    test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");

    test_setup.start_block();
    for _ in 0..n_change_pubkeys {
        test_setup
            .change_pubkey_with_onchain_auth(
                ETHAccountId(0),
                ZKSyncAccountId(1),
                token,
                0u32.into(),
            )
            .await;
    }
    let change_pubkey_execute_result = test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");
    assert_eq!(
        change_pubkey_execute_result.block_size_chunks,
        n_change_pubkeys * ChangePubKeyOp::CHUNKS,
        "block size mismatch"
    );
    CostsSample::new(
        n_change_pubkeys,
        U256::from(0),
        change_pubkey_execute_result,
    )
}

async fn commit_cost_of_create2_change_pubkey(
    test_setup: &mut TestSetup,
    n_change_pubkeys: usize,
) -> CostsSample {
    let rng = &mut rand::thread_rng();
    let token = Token(TokenId(0));
    let fee_amount = 100u32;
    let deposit_amount = (fee_amount * (n_change_pubkeys + 1) as u32).into();

    let first_new_account_id = test_setup.accounts.zksync_accounts.len();

    test_setup.start_block();
    test_setup
        .deposit(ETHAccountId(1), ZKSyncAccountId(1), token, deposit_amount)
        .await;
    test_setup
        .change_pubkey_with_tx(ZKSyncAccountId(1), token, 0u32.into())
        .await;
    for new_account_idx in 0..n_change_pubkeys {
        let pk = priv_key_from_fs(rng.gen());
        let create2_data = ChangePubKeyCREATE2Data {
            creator_address: Address::random(),
            code_hash: H256::random(),
            salt_arg: H256::random(),
        };
        let pubkey_hash = PubKeyHash::from_privkey(&pk);
        let address = create2_data.get_address(&pubkey_hash);
        let zksync_account = ZkSyncAccount::new(
            pk,
            Nonce(0),
            address,
            ZkSyncETHAccountData::Create2(create2_data),
        );
        test_setup
            .accounts
            .zksync_accounts
            .insert(first_new_account_id + new_account_idx, zksync_account);
        test_setup
            .transfer(
                ZKSyncAccountId(1),
                ZKSyncAccountId(first_new_account_id + new_account_idx),
                token,
                0u32.into(),
                0u32.into(),
                Default::default(),
            )
            .await;
    }
    test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");

    test_setup.start_block();
    for new_account_idx in 0..n_change_pubkeys {
        test_setup
            .change_pubkey_with_tx(
                ZKSyncAccountId(first_new_account_id + new_account_idx),
                token,
                0u32.into(),
            )
            .await;
    }
    let change_pubkey_execute_result = test_setup
        .execute_commit_and_verify_block()
        .await
        .expect("Block execution failed");
    assert_eq!(
        change_pubkey_execute_result.block_size_chunks,
        n_change_pubkeys * ChangePubKeyOp::CHUNKS,
        "block size mismatch"
    );
    CostsSample::new(
        n_change_pubkeys,
        U256::from(0),
        change_pubkey_execute_result,
    )
}

#[tokio::main]
async fn main() {
    gas_price_test().await;
}
