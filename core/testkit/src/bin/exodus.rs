use testkit::*;
use models::config_options::ConfigurationOptions;
use models::EncodedProof;
use std::time::Instant;
use std::thread::JoinHandle;
use web3::transports::Http;
use web3::types::{Address, U64, U128, U256};
use web3::Transport;
use bigdecimal::BigDecimal;
use bigdecimal::ToPrimitive;
use models::node::{
    Account, AccountId, AccountMap, FranklinTx, Nonce, PriorityOp, TokenId,
};
use crate::eth_account::{parse_ether, EthereumAccount};
use crate::external_commands::{deploy_test_contracts, get_test_accounts, Contracts, get_revert_reason};
use crate::zksync_account::ZksyncAccount;

use futures::{
    channel::{mpsc, oneshot},
    executor::block_on,
    SinkExt, StreamExt,
};


fn get_exit_proof(account: AccountId, token: TokenId) -> Option<EncodedProof> {
    // get tree of accounts
    // get hashes of account and token
    // get proof 
    // 
    // return proof 
    // uint256[8] calldata in solidity
    // uint256[8] calldata _proof 
    // pub type EncodedProof = [U256; 8];
    // 
    Some(EncodedProof::default())
}

fn exit_test() {
    let config = ConfigurationOptions::from_env();

    let fee_account = ZksyncAccount::rand();
    let (sk_thread_handle, stop_state_keeper_sender, sk_channels) =
        spawn_state_keeper(&fee_account.address);

    let deploy_timer = Instant::now();
    println!("deploying contracts");
    let contracts = deploy_test_contracts();
    println!(
        "contracts deployed {:#?}, {} secs",
        contracts,
        deploy_timer.elapsed().as_secs()
    );

    let (_el, transport) = Http::new(&config.web3_url).expect("http transport start");
    let commit_account = EthereumAccount::new(
        config.operator_private_key,
        config.operator_eth_addr,
        transport.clone(),
        contracts.contract,
        &config,
    );

    let eth_accounts = get_test_accounts()
        .into_iter()
        .map(|test_eth_account| {
            EthereumAccount::new(
                test_eth_account.private_key,
                test_eth_account.address,
                transport.clone(),
                contracts.contract,
                &config,
            )
        })
        .collect::<Vec<_>>();

    let accounts = AccountSet {
        eth_accounts,
        zksync_accounts: vec![
            fee_account,
            ZksyncAccount::rand(),
            ZksyncAccount::rand(),
            ZksyncAccount::rand(),
        ],
        fee_account_id: ZKSyncAccountId(0),
    };

    let mut test_setup = TestSetup::new(sk_channels, accounts, &contracts, commit_account);

    let deposit_amount = parse_ether("0.1").unwrap();

    let account_ids: Vec<usize> = vec![0, 1];
    let token_ids: Vec<TokenId> = vec![0, 1];

    println!("### Create some initial state, verify");
    for token in token_ids.clone() {
        test_setup.start_block();
        for account in account_ids.clone() {
            test_setup.deposit(
                ETHAccountId(0),
                ZKSyncAccountId(account),
                Token(token),
                deposit_amount.clone(),
            );
        }
        test_setup.execute_commit_and_verify_block();
    }

    // Then trigger exodus: 
    println!("### send some deposits, but don't verify them");
    let trigger_exodus_deposit_account_ids = vec![0];
    let num_sent_deposits = {
        let mut sent_deposits_count = 0;
        loop {
            test_setup.start_block();
            for account in trigger_exodus_deposit_account_ids.clone() {
                for token in token_ids.clone() {
                    test_setup.deposit(
                        ETHAccountId(account),
                        ZKSyncAccountId(account),
                        Token(token),
                        deposit_amount.clone(),
                    );
                }
            }
            println!("total_blocks_committed: {}", test_setup.total_blocks_committed().unwrap());
            if let Ok(reason) = test_setup.execute_commit_block() {
                if reason == "tx success" {
                    sent_deposits_count += 1;
                }
            }

            if test_setup.is_exodus().unwrap() {
                println!("Finally exodus'");
                break;
            } else {
                println!("Not yet exodus, oh");
            }
        }
        sent_deposits_count
    };

    let balance_from_cancel_deposits = &deposit_amount * BigDecimal::from(num_sent_deposits);

    println!("### We managed to send {} deposits totalling {}, let's try to cancel them", &num_sent_deposits, &balance_from_cancel_deposits);
    block_on(async {
        println!("cancelDeposits: {}", test_setup.accounts.eth_accounts[0].cancel_outstanding_deposits_for_exodus_mode(0).await.unwrap());
        println!("cancelDeposits: {}", test_setup.accounts.eth_accounts[0]
            .cancel_outstanding_deposits_for_exodus_mode(num_sent_deposits + 20).await.unwrap());

        for account in trigger_exodus_deposit_account_ids.clone() {
            for token in token_ids.clone() {
                let bal = test_setup.get_balance_to_withdraw_async(ETHAccountId(account), token).await;
                println!("bal {}", &bal);
                assert!(bal == balance_from_cancel_deposits);
            }
        }
    });

    println!("### Now call exit for every account:");
    block_on(async {
        for account in account_ids.clone()  /* as AccountId  */{
            for token in token_ids.clone() {
                let balance_before: BigDecimal = 
                    test_setup.get_balance_to_withdraw_async(ETHAccountId(account), token).await;
                
                if let Err(payload) = test_setup.exit(
                    ETHAccountId(account),
                    token,
                    deposit_amount.to_u128().unwrap(),
                    get_exit_proof(account as AccountId, token).unwrap(),
                ).await {
                    println!("{:?}", payload);
                }

                let balances_after = 
                    test_setup.get_balance_to_withdraw_async(ETHAccountId(account), token).await;
                println!("account {}, token {}, balances_after {}, balance_before {}", &account, &token, &balances_after, &balance_before);
                
                assert!(balances_after - balance_before == deposit_amount);
            }
        }
    });

    println!("### Ok, but can a user call exit twice and still get money?");
    block_on(async {
        for account in account_ids.clone() {
            for token in token_ids.clone() {
                let balance_before: BigDecimal = 
                    test_setup.get_balance_to_withdraw_async(ETHAccountId(account), token).await;

                for _ in 0..2 {
                    // first it should bump from 0 to deposit_amount, then shouldn't change
                    test_setup.exit(
                        ETHAccountId(account),
                        token,
                        deposit_amount.to_u128().unwrap(),
                        get_exit_proof(account as AccountId, token).unwrap(),
                    ).await;
                }

                assert!(
                    test_setup.get_balance_to_withdraw_async(
                        ETHAccountId(account), token
                    ).await == balance_before
                );
            }
        }
    });

    println!("### try to withdraw not real user balance, should fail.");
    block_on(async {
        // nobody has any balance
        for account in account_ids.clone() {
            for token in token_ids.clone() {
                assert!(
                    test_setup.get_balance_to_withdraw_async(
                        ETHAccountId(account), token
                    ).await == BigDecimal::from(0)
                );
            }
        }

        let account = 0;
        for token in token_ids.clone() {
            for _ in 0..3 {
                test_setup.exit(
                    ETHAccountId(account),
                    token,
                    (&deposit_amount + &deposit_amount).to_u128().unwrap(),
                    get_exit_proof(account as AccountId, token).unwrap(),
                ).await;
                assert!(
                    test_setup.get_balance_to_withdraw_async(
                        ETHAccountId(account), token
                    ).await == BigDecimal::from(0)
                );
            }
        }
    });

    stop_state_keeper_sender.send(()).expect("sk stop send");
    sk_thread_handle.join().expect("sk thread join");
}

fn main() {
    exit_test();
}
