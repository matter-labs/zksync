use crate::eth_account::{parse_ether, EthereumAccount};
use crate::external_commands::{deploy_test_contracts, get_test_accounts};
use crate::zksync_account::ZksyncAccount;
use models::node::BlockTimestamp;
use std::time::SystemTime;
use testkit::*;
use web3::transports::Http;

pub enum BlockTimestampTestScenario {
    BlocksTimestampsDependency,
    VerySmallTimestamp,
    VeryBigTimestamp,
    ExpiredTimestamp,
}

fn block_timestamp_test(scenario: BlockTimestampTestScenario) {
    let testkit_config = get_testkit_config_from_env();

    let fee_account = ZksyncAccount::rand();
    let (sk_thread_handle, stop_state_keeper_sender, sk_channels) =
        spawn_state_keeper(&fee_account.address);

    let contracts = deploy_test_contracts();

    let (_el, transport) = Http::new(&testkit_config.web3_url).expect("http transport start");
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
        let mut zksync_accounts = Vec::new();
        zksync_accounts.push(fee_account);
        zksync_accounts.extend(eth_accounts.iter().map(|eth_account| {
            let rng_zksync_key = ZksyncAccount::rand().private_key;
            ZksyncAccount::new(
                rng_zksync_key,
                0,
                eth_account.address,
                eth_account.private_key,
            )
        }));
        zksync_accounts
    };

    let accounts = AccountSet {
        eth_accounts,
        zksync_accounts,
        fee_account_id: ZKSyncAccountId(0),
    };

    let mut test_setup = TestSetup::new(sk_channels, accounts, &contracts, commit_account);

    let deposit_amount = parse_ether("1.0").unwrap();

    match scenario {
        BlockTimestampTestScenario::BlocksTimestampsDependency => {
            // some ok block
            test_setup.start_block();
            test_setup.deposit(
                ETHAccountId(0),
                ZKSyncAccountId(2),
                Token(0),
                deposit_amount.clone(),
            );
            test_setup.execute_commit_block().expect_success();

            // should fail because of blocks timestamps dependency
            test_setup.start_block();
            test_setup.deposit(
                ETHAccountId(0),
                ZKSyncAccountId(2),
                Token(0),
                deposit_amount,
            );
            test_setup
                .execute_commit_block_with_defined_timestamp(BlockTimestamp::from(0))
                .expect_revert("com91");
            println!("blocks timestamps dependency works correctly");
        }
        BlockTimestampTestScenario::VerySmallTimestamp => {
            // should fail because of small timestamp
            test_setup.start_block();
            test_setup.deposit(
                ETHAccountId(0),
                ZKSyncAccountId(2),
                Token(0),
                deposit_amount,
            );
            test_setup
                .execute_commit_block_with_defined_timestamp(BlockTimestamp::from(
                    u64::min_value() + 1,
                ))
                .expect_revert("com91");
            println!("small timestamp will not be passed to the verifier");
        }
        BlockTimestampTestScenario::VeryBigTimestamp => {
            // should fail because of big timestamp
            test_setup.start_block();
            test_setup.deposit(
                ETHAccountId(0),
                ZKSyncAccountId(2),
                Token(0),
                deposit_amount,
            );
            test_setup
                .execute_commit_block_with_defined_timestamp(BlockTimestamp::from(u64::max_value()))
                .expect_revert("com91");
            println!("big timestamp will not be passed to the verifier");
        }
        BlockTimestampTestScenario::ExpiredTimestamp => {
            // should fail because of expired timestamp
            test_setup.start_block();
            test_setup.deposit(
                ETHAccountId(0),
                ZKSyncAccountId(2),
                Token(0),
                deposit_amount,
            );
            let current_timestamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("unix timestamp calculation failed")
                .as_secs();
            let one_week = 7 * 24 * 60 * 60;
            test_setup
                .execute_commit_block_with_defined_timestamp(BlockTimestamp::from(
                    current_timestamp - one_week - 1,
                ))
                .expect_revert("com91");
            println!("an expired timestamp will not be passed to the verifier");
        }
    };

    stop_state_keeper_sender.send(()).expect("sk stop send");
    sk_thread_handle.join().expect("sk thread join");
}

fn main() {
    block_timestamp_test(BlockTimestampTestScenario::BlocksTimestampsDependency);
    block_timestamp_test(BlockTimestampTestScenario::VerySmallTimestamp);
    block_timestamp_test(BlockTimestampTestScenario::VeryBigTimestamp);
    block_timestamp_test(BlockTimestampTestScenario::ExpiredTimestamp);
}
