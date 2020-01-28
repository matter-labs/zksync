use testkit::*;
use models::config_options::ConfigurationOptions;
use models::EncodedProof;
use std::time::Instant;
use web3::transports::Http;
use web3::types::{Address, U64, U128, U256};
use web3::Transport;
use bigdecimal::BigDecimal;
use bigdecimal::ToPrimitive;
use models::node::{
    Account, AccountAddress, AccountId, AccountMap, FranklinTx, Nonce, PriorityOp, TokenId,
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

    // create some initial state, verify
    for token in 0..=1 {
        println!("token {}", &token);
        test_setup.start_block();
        for account in 0..1 {
            test_setup.deposit(
                ETHAccountId(0),
                ZKSyncAccountId(account),
                Token(token),
                deposit_amount.clone(),
            );
        }
        test_setup.execute_commit_and_verify_block();
    }

    // trigger exodus (
    // send at least one deposit, 
    // commit a lot of blocks,
    // verify none
    // )
    for block_n in 0..20 {
        test_setup.start_block();
        test_setup.deposit(
            ETHAccountId(0),
            ZKSyncAccountId(0),
            Token(0),
            deposit_amount.clone(),
        );
        println!("total_blocks_committed: {}", test_setup.total_blocks_committed().unwrap());
        test_setup.execute_commit_block();

        if test_setup.is_exodus().unwrap() {
            println!("Finally exodus'");
            break;
        } else {
            println!("Not yet exodus, oh");
        }
    }


    // after a lot unverified blocks,
    // state doesn't change by transactions and withdraws.
    // But it is changed by deposits. So count deposit amounts.
    // And call cancelOutstandingDeposits
    // and check the balances to withdraw.
    // 
    // After that, for every balance in last verified state,
    // call exit()

    block_on(async {
        for account in 0..1 {
            println!(
                "{:?}", 
                test_setup
                    .accounts
                    .eth_accounts[account]
                    .balances_to_withdraw(0)
                    .await
            );

            test_setup.exit(
                ETHAccountId(account),
                0,
                deposit_amount.to_u128().unwrap(),
                get_exit_proof(0, 0).unwrap(),
            ).await;

            println!(
                "{:?}", 
                test_setup
                    .accounts
                    .eth_accounts[account]
                    .balances_to_withdraw(0)
                    .await
            );
        }
    });

    stop_state_keeper_sender.send(()).expect("sk stop send");
    sk_thread_handle.join().expect("sk thread join");
}

fn main() {
    exit_test();
}
