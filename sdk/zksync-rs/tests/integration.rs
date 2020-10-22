//! Integration test for zkSync Rust SDK.
//!
//! In order to pass these tests, there must be a running
//! instance of zkSync server and prover:
//!
//! ```bash
//! zksync server &!
//! zksync dummy-prover &!
//! zksync sdk-test
//! ```
//!
//! Note: If tests are failing, first check the following two things:
//!
//! 1. If tests are failing with an error "cannot operate after unexpected tx failure",
//!    ensure that dummy prover is enabled.
//! 2. If tests are failing with an error "replacement transaction underpriced",
//!    ensure that tests are ran in one thread. Running the tests with many threads won't
//!    work, since many thread will attempt in sending transactions from one (main) Ethereum
//!    account, which may result in nonce mismatch.
//!    Also, if there will be many tests running at once, and the server will die, it will be
//!    hard to distinguish which test exactly caused this problem.

use std::time::{Duration, Instant};
use zksync::operations::SyncTransactionHandle;
use zksync::{
    error::ClientError,
    types::BlockStatus,
    web3::{
        contract::{Contract, Options},
        transports::Http,
        types::{Address, H160, H256, U256},
    },
    zksync_types::{tx::PackedEthSignature, Token, TokenLike, TxFeeTypes, ZkSyncTx},
    EthereumProvider, Network, Provider, Wallet, WalletCredentials,
};
use zksync_contracts::{erc20_contract, zksync_contract};
use zksync_eth_signer::{EthereumSigner, PrivateKeySigner};

const ETH_ADDR: &str = "36615Cf349d7F6344891B1e7CA7C72883F5dc049";
const ETH_PRIVATE_KEY: &str = "7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110";
const LOCALHOST_WEB3_ADDR: &str = "http://127.0.0.1:8545";

fn eth_main_account_credentials() -> (H160, H256) {
    let addr = ETH_ADDR.parse().unwrap();
    let eth_private_key = ETH_PRIVATE_KEY.parse().unwrap();

    (addr, eth_private_key)
}

fn eth_random_account_credentials() -> (H160, H256) {
    let mut eth_private_key = H256::default();
    eth_private_key.randomize();

    let address_from_pk = PackedEthSignature::address_from_private_key(&eth_private_key).unwrap();

    (address_from_pk, eth_private_key)
}

fn one_ether() -> U256 {
    U256::from(10).pow(18.into())
}

/// Auxiliary function that returns the balance of the account on Ethereum.
async fn get_ethereum_balance<S: EthereumSigner + Clone>(
    eth_provider: &EthereumProvider<S>,
    address: Address,
    token: &Token,
) -> Result<U256, anyhow::Error> {
    if token.symbol == "ETH" {
        return eth_provider
            .web3()
            .eth()
            .balance(address, None)
            .await
            .map_err(|_e| anyhow::anyhow!("failed to request balance from Ethereum {}", _e));
    }

    let contract = Contract::new(eth_provider.web3().eth(), token.address, erc20_contract());
    contract
        .query("balanceOf", address, None, Options::default(), None)
        .await
        .map_err(|_e| anyhow::anyhow!("failed to request erc20 balance from Ethereum"))
}

async fn wait_for_deposit_and_update_account_id<S: EthereumSigner + Clone>(wallet: &mut Wallet<S>) {
    let timeout = Duration::from_secs(60);
    let mut poller = tokio::time::interval(std::time::Duration::from_millis(100));
    let start = Instant::now();
    while wallet
        .provider
        .account_info(wallet.address())
        .await
        .unwrap()
        .id
        .is_none()
    {
        if start.elapsed() > timeout {
            panic!("Timeout elapsed while waiting for Ethereum transaction");
        }
        poller.tick().await;
    }

    wallet.update_account_id().await.unwrap();
    assert!(wallet.account_id().is_some(), "Account ID was not set");
}

async fn transfer_to(
    token_like: impl Into<TokenLike>,
    amount: impl Into<U256>,
    to: H160,
) -> Result<(), anyhow::Error> {
    let (main_eth_address, main_eth_private_key) = eth_main_account_credentials();

    let provider = Provider::new(Network::Localhost);
    let eth_signer = PrivateKeySigner::new(main_eth_private_key);
    let credentials =
        WalletCredentials::from_eth_signer(main_eth_address, eth_signer, Network::Localhost)
            .await
            .unwrap();

    let wallet = Wallet::new(provider, credentials).await?;
    let ethereum = wallet.ethereum(LOCALHOST_WEB3_ADDR).await?;
    let hash = ethereum
        .transfer(token_like.into(), amount.into(), to)
        .await
        .unwrap();

    ethereum.wait_for_tx(hash).await?;
    Ok(())
}

/// Creates a new wallet and tries to make a transfer
/// from a new wallet without SigningKey.
async fn test_tx_fail<S: EthereumSigner + Clone>(
    zksync_depositor_wallet: &Wallet<S>,
) -> Result<(), anyhow::Error> {
    let provider = Provider::new(Network::Localhost);

    let (random_eth_address, random_eth_private_key) = eth_random_account_credentials();
    let eth_signer = PrivateKeySigner::new(random_eth_private_key);
    let random_credentials =
        WalletCredentials::from_eth_signer(random_eth_address, eth_signer, Network::Localhost)
            .await?;
    let sync_wallet = Wallet::new(provider, random_credentials).await?;

    let handle = sync_wallet
        .start_transfer()
        .to(zksync_depositor_wallet.address())
        .token("ETH")?
        .amount(1_000_000u64)
        .send()
        .await;

    assert!(matches!(
        handle,
        Err(ClientError::SigningError(_no_signing_key))
    ));

    Ok(())
}

/// Checks the correctness of the `Deposit` operation.
async fn test_deposit<S: EthereumSigner + Clone>(
    deposit_wallet: &Wallet<S>,
    sync_wallet: &mut Wallet<S>,
    token: &Token,
    amount: u128,
) -> Result<(), anyhow::Error> {
    let ethereum = deposit_wallet.ethereum(LOCALHOST_WEB3_ADDR).await?;

    if !deposit_wallet.tokens.is_eth(token.address.into()) {
        if !ethereum.is_erc20_deposit_approved(token.address).await? {
            let tx_approve_deposits = ethereum.approve_erc20_token_deposits(token.address).await?;
            ethereum.wait_for_tx(tx_approve_deposits).await?;
        }

        assert!(
            ethereum.is_erc20_deposit_approved(token.address).await?,
            "Token should be approved"
        );
    };

    // let balance_before = sync_wallet.get_balance(BlockStatus::Committed, &token.symbol as &str).await?;
    let deposit_tx_hash = ethereum
        .deposit(
            &token.symbol as &str,
            U256::from(amount),
            sync_wallet.address(),
        )
        .await?;

    ethereum.wait_for_tx(deposit_tx_hash).await?;
    wait_for_deposit_and_update_account_id(sync_wallet).await;

    // let balance_after = sync_wallet.get_balance(BlockStatus::Committed, &token.symbol as &str).await?;

    if !sync_wallet.tokens.is_eth(token.address.into()) {
        assert!(ethereum.is_erc20_deposit_approved(token.address).await?);
    }

    // To be sure that the deposit is committed, we need to listen to the event `NewPriorityRequest`
    // rust SDK doesn't support getting this information yet, but it will be added soon.
    // assert_eq!(balance_after - balance_before, u256_to_big_dec(amount / 2));

    Ok(())
}

/// Checks the correctness of the `ChangePubKey` operation.
async fn test_change_pubkey<S: EthereumSigner + Clone>(
    sync_wallet: &Wallet<S>,
    token_symbol: &str,
) -> Result<(), anyhow::Error> {
    if !sync_wallet.is_signing_key_set().await? {
        let handle = sync_wallet
            .start_change_pubkey()
            .fee_token(token_symbol)?
            .send()
            .await?;

        handle
            .commit_timeout(Duration::from_secs(60))
            .wait_for_commit()
            .await?;
    }
    assert!(sync_wallet.is_signing_key_set().await?);
    Ok(())
}

/// Makes a transfer from Alice to Bob inside zkSync
/// checks the correctness of the amount of money before the transaction and after.
async fn test_transfer<S: EthereumSigner + Clone>(
    alice: &Wallet<S>,
    bob: &Wallet<S>,
    token_symbol: &str,
    transfer_amount: u128,
) -> Result<(), anyhow::Error> {
    let transfer_amount = num::BigUint::from(transfer_amount);

    let total_fee = alice
        .provider
        .get_tx_fee(TxFeeTypes::Transfer, bob.address(), token_symbol)
        .await?
        .total_fee;

    let alice_balance_before = alice
        .get_balance(BlockStatus::Committed, token_symbol)
        .await?;

    let bob_balance_before = bob
        .get_balance(BlockStatus::Committed, token_symbol)
        .await?;

    let transfer_handle = alice
        .start_transfer()
        .to(bob.address())
        .token(token_symbol)?
        .amount(transfer_amount.clone())
        .send()
        .await?;

    transfer_handle
        .verify_timeout(Duration::from_secs(180))
        .wait_for_verify()
        .await?;

    let alice_balance_after = alice
        .get_balance(BlockStatus::Committed, token_symbol)
        .await?;
    let bob_balance_after = bob
        .get_balance(BlockStatus::Committed, token_symbol)
        .await?;

    assert_eq!(
        alice_balance_before - alice_balance_after,
        transfer_amount.clone() + total_fee
    );
    assert_eq!(bob_balance_after - bob_balance_before, transfer_amount);
    Ok(())
}

/// Makes a transaction from the account to its own address
/// checks if the expected amount of fee has been spent.
async fn test_transfer_to_self<S: EthereumSigner + Clone>(
    sync_wallet: &Wallet<S>,
    token_symbol: &str,
    transfer_amount: u128,
) -> Result<(), anyhow::Error> {
    let transfer_amount = num::BigUint::from(transfer_amount);
    let balance_before = sync_wallet
        .get_balance(BlockStatus::Committed, token_symbol)
        .await?;
    let total_fee = sync_wallet
        .provider
        .get_tx_fee(TxFeeTypes::Transfer, sync_wallet.address(), token_symbol)
        .await?
        .total_fee;

    let transfer_handle = sync_wallet
        .start_transfer()
        .to(sync_wallet.address())
        .token(token_symbol)?
        .amount(transfer_amount)
        .send()
        .await?;

    transfer_handle
        .verify_timeout(Duration::from_secs(180))
        .wait_for_verify()
        .await?;

    let balance_after = sync_wallet
        .get_balance(BlockStatus::Committed, token_symbol)
        .await?;

    assert_eq!(balance_before - balance_after, total_fee);

    Ok(())
}

/// Makes a withdraw operation on L2
/// checks the correctness of their execution.
async fn test_withdraw<S: EthereumSigner + Clone>(
    eth_provider: &EthereumProvider<S>,
    main_contract: &Contract<Http>,
    sync_wallet: &Wallet<S>,
    withdraw_to: &Wallet<S>,
    token: &Token,
    amount: u128,
) -> Result<(), anyhow::Error> {
    let total_fee = sync_wallet
        .provider
        .get_tx_fee(TxFeeTypes::Withdraw, withdraw_to.address(), token.address)
        .await?
        .total_fee;
    let sync_balance_before = sync_wallet
        .get_balance(BlockStatus::Committed, &token.symbol as &str)
        .await?;
    let onchain_balance_before =
        get_ethereum_balance(eth_provider, withdraw_to.address(), token).await?;
    let pending_to_be_onchain_balance_before: U256 = {
        let query = main_contract.query(
            "getBalanceToWithdraw",
            (withdraw_to.address(), token.id),
            None,
            Options::default(),
            None,
        );

        query
            .await
            .map_err(|err| anyhow::anyhow!(format!("Contract query fail: {}", err)))?
    };

    let withdraw_handle = sync_wallet
        .start_withdraw()
        .to(withdraw_to.address())
        .token(token.address)?
        .amount(amount)
        .send()
        .await?;

    withdraw_handle
        .verify_timeout(Duration::from_secs(180))
        .wait_for_verify()
        .await?;

    let sync_balance_after = sync_wallet
        .get_balance(BlockStatus::Committed, &token.symbol as &str)
        .await?;
    let onchain_balance_after =
        get_ethereum_balance(eth_provider, withdraw_to.address(), token).await?;

    let pending_to_be_onchain_balance_after: U256 = {
        let query = main_contract.query(
            "getBalanceToWithdraw",
            (withdraw_to.address(), token.id),
            None,
            Options::default(),
            None,
        );

        query
            .await
            .map_err(|err| anyhow::anyhow!(format!("Contract query fail: {}", err)))?
    };

    assert_eq!(
        onchain_balance_after - onchain_balance_before + pending_to_be_onchain_balance_after
            - pending_to_be_onchain_balance_before,
        U256::from(amount)
    );
    assert_eq!(
        sync_balance_before - sync_balance_after,
        num::BigUint::from(amount) + total_fee
    );

    Ok(())
}

/// Makes transfers for different types of operations
/// checks the correctness of their execution.
async fn move_funds<S: EthereumSigner + Clone>(
    main_contract: &Contract<Http>,
    eth_provider: &EthereumProvider<S>,
    depositor_wallet: &Wallet<S>,
    alice: &mut Wallet<S>,
    bob: &Wallet<S>,
    token_like: impl Into<TokenLike>,
    deposit_amount: u128,
) -> Result<(), anyhow::Error> {
    let token_like = token_like.into();
    let token = depositor_wallet
        .tokens
        .resolve(token_like.clone())
        .ok_or_else(|| anyhow::anyhow!("Error resolve token"))?;

    let transfer_amount = deposit_amount / 10;
    let withdraw_amount = deposit_amount / 10;

    test_deposit(depositor_wallet, alice, &token, deposit_amount).await?;
    println!("Deposit ok, Token: {}", token.symbol);

    test_change_pubkey(alice, &token.symbol).await?;
    println!("Change pubkey ok");

    test_transfer(alice, bob, &token.symbol, transfer_amount).await?;
    println!("Transfer to new ok, Token: {}", token.symbol);

    test_transfer(alice, bob, &token.symbol, transfer_amount).await?;
    println!("Transfer ok, Token: {}", token.symbol);

    test_transfer_to_self(&alice, &token.symbol, transfer_amount).await?;
    println!("Transfer to self ok, Token: {}", token.symbol);

    test_withdraw(
        &eth_provider,
        &main_contract,
        &alice,
        &bob,
        &token,
        withdraw_amount,
    )
    .await?;
    println!("Withdraw ok, Token: {}", token.symbol);

    // Currently fast withdraw aren't supported by zksync-rs, but they will be in the near future.
    // test_fast_withdraw(eth, main_contract, &bob, &bob, &token, withdraw_amount);
    // println!("Fast withdraw ok, Token: {}", token.symbol);

    // Currently multi transactions aren't supported by zksync-rs, but they will be in the near future.
    // test_multi_transfer(alice, bob, &token.symbol, transfersAmount / 2);
    // println!("Batched transfers ok, Token: {}, token.symbol");

    Ok(())
}

/// Auxiliary function that generates a new wallet, performs an initial deposit and changes the public key.
async fn init_account_with_one_ether() -> Result<Wallet<PrivateKeySigner>, anyhow::Error> {
    let (eth_address, eth_private_key) = eth_random_account_credentials();

    // Transfer funds from "rich" account to a randomly created one (so we won't reuse the same
    // account in subsequent test runs).
    transfer_to("ETH", one_ether(), eth_address).await?;

    let provider = Provider::new(Network::Localhost);

    let eth_signer = PrivateKeySigner::new(eth_private_key);
    let credentials =
        WalletCredentials::from_eth_signer(eth_address, eth_signer, Network::Localhost)
            .await
            .unwrap();

    let mut wallet = Wallet::new(provider, credentials).await?;
    let ethereum = wallet.ethereum(LOCALHOST_WEB3_ADDR).await?;

    let deposit_tx_hash = ethereum
        .deposit("ETH", one_ether() / 2, wallet.address())
        .await?;

    ethereum.wait_for_tx(deposit_tx_hash).await?;

    // Update stored wallet ID after we initialized a wallet via deposit.
    wait_for_deposit_and_update_account_id(&mut wallet).await;

    if !wallet.is_signing_key_set().await? {
        let handle = wallet
            .start_change_pubkey()
            .fee_token("ETH")?
            .send()
            .await?;

        handle
            .commit_timeout(Duration::from_secs(60))
            .wait_for_commit()
            .await?;
    }

    Ok(wallet)
}

#[tokio::test]
#[cfg_attr(not(feature = "integration-tests"), ignore)]
async fn comprehensive_test() -> Result<(), anyhow::Error> {
    let provider = Provider::new(Network::Localhost);

    let main_wallet = {
        let (main_eth_address, main_eth_private_key) = eth_main_account_credentials();
        let eth_signer = PrivateKeySigner::new(main_eth_private_key);
        let main_credentials =
            WalletCredentials::from_eth_signer(main_eth_address, eth_signer, Network::Localhost)
                .await?;
        Wallet::new(provider.clone(), main_credentials).await?
    };

    let sync_depositor_wallet = {
        let (random_eth_address, random_eth_private_key) = eth_random_account_credentials();
        let eth_signer = PrivateKeySigner::new(random_eth_private_key);
        let random_credentials =
            WalletCredentials::from_eth_signer(random_eth_address, eth_signer, Network::Localhost)
                .await?;
        Wallet::new(provider.clone(), random_credentials).await?
    };

    let mut alice_wallet1 = {
        let (random_eth_address, random_eth_private_key) = eth_random_account_credentials();
        let eth_signer = PrivateKeySigner::new(random_eth_private_key);
        let random_credentials =
            WalletCredentials::from_eth_signer(random_eth_address, eth_signer, Network::Localhost)
                .await?;
        Wallet::new(provider.clone(), random_credentials).await?
    };

    let mut alice_wallet2 = {
        let (random_eth_address, random_eth_private_key) = eth_random_account_credentials();
        let eth_signer = PrivateKeySigner::new(random_eth_private_key);
        let random_credentials =
            WalletCredentials::from_eth_signer(random_eth_address, eth_signer, Network::Localhost)
                .await?;
        Wallet::new(provider.clone(), random_credentials).await?
    };

    let bob_wallet1 = {
        let (random_eth_address, random_eth_private_key) = eth_random_account_credentials();
        let eth_signer = PrivateKeySigner::new(random_eth_private_key);
        let random_credentials =
            WalletCredentials::from_eth_signer(random_eth_address, eth_signer, Network::Localhost)
                .await?;
        Wallet::new(provider.clone(), random_credentials).await?
    };

    let bob_wallet2 = {
        let (random_eth_address, random_eth_private_key) = eth_random_account_credentials();
        let eth_signer = PrivateKeySigner::new(random_eth_private_key);
        let random_credentials =
            WalletCredentials::from_eth_signer(random_eth_address, eth_signer, Network::Localhost)
                .await?;
        Wallet::new(provider.clone(), random_credentials).await?
    };

    let ethereum = main_wallet.ethereum(LOCALHOST_WEB3_ADDR).await?;

    let main_contract = {
        let address_response = provider.contract_address().await?;
        let contract_address = if address_response.main_contract.starts_with("0x") {
            &address_response.main_contract[2..]
        } else {
            &address_response.main_contract
        }
        .parse()?;

        Contract::new(ethereum.web3().eth(), contract_address, zksync_contract())
    };

    let token_eth = sync_depositor_wallet
        .tokens
        .resolve("ETH".into())
        .ok_or_else(|| anyhow::anyhow!("Error resolve token"))?;

    let token_dai = sync_depositor_wallet
        .tokens
        .resolve("DAI".into())
        .ok_or_else(|| anyhow::anyhow!("Error resolve token"))?;

    let eth_deposit_amount = U256::from(10).pow(18.into()) * 6; // 6 Ethers
    let dai_deposit_amount = U256::from(10).pow(18.into()) * 10000; // 10000 DAI

    transfer_to("ETH", eth_deposit_amount, sync_depositor_wallet.address()).await?;
    transfer_to("DAI", dai_deposit_amount, sync_depositor_wallet.address()).await?;

    assert_eq!(
        get_ethereum_balance(&ethereum, sync_depositor_wallet.address(), &token_eth).await?,
        eth_deposit_amount
    );

    assert_eq!(
        get_ethereum_balance(&ethereum, sync_depositor_wallet.address(), &token_dai).await?,
        dai_deposit_amount
    );

    test_tx_fail(&sync_depositor_wallet).await?;

    move_funds(
        &main_contract,
        &ethereum,
        &sync_depositor_wallet,
        &mut alice_wallet1,
        &bob_wallet1,
        "DAI",
        // 200 DAI
        200_000_000_000_000_000_000u128,
    )
    .await?;

    move_funds(
        &main_contract,
        &ethereum,
        &sync_depositor_wallet,
        &mut alice_wallet2,
        &bob_wallet2,
        "ETH",
        // 1 Ether (10^18 WEI)
        1_000_000_000_000_000_000u128,
    )
    .await?;

    Ok(())
}

#[tokio::test]
#[cfg_attr(not(feature = "integration-tests"), ignore)]
async fn simple_transfer() -> Result<(), anyhow::Error> {
    let wallet = init_account_with_one_ether().await?;

    // Perform a transfer to itself.
    let handle = wallet
        .start_transfer()
        .to(wallet.signer.address)
        .token("ETH")?
        .amount(1_000_000u64)
        .send()
        .await?;

    handle
        .verify_timeout(Duration::from_secs(180))
        .wait_for_verify()
        .await?;

    Ok(())
}

#[tokio::test]
#[cfg_attr(not(feature = "integration-tests"), ignore)]
async fn batch_transfer() -> Result<(), anyhow::Error> {
    let wallet = init_account_with_one_ether().await?;

    const RECIPIENT_COUNT: usize = 4;
    let recipients = vec![eth_random_account_credentials().0; RECIPIENT_COUNT];

    let token_like = TokenLike::Symbol("ETH".to_owned());
    let token = wallet
        .tokens
        .resolve(token_like.clone())
        .expect("ETH token resolving failed");

    let mut nonce = wallet.account_info().await?.committed.nonce;

    // Sign a transfer for each recipient created above
    let mut signed_transfers = Vec::with_capacity(recipients.len());

    for recipient in recipients {
        let fee = wallet
            .provider
            .get_tx_fee(TxFeeTypes::Transfer, recipient, token_like.clone())
            .await?
            .total_fee;

        let (transfer, signature) = wallet
            .signer
            .sign_transfer(token.clone(), 1_000_000u64.into(), fee, recipient, nonce)
            .await
            .expect("Transfer signing error");

        signed_transfers.push((ZkSyncTx::Transfer(Box::new(transfer)), signature));

        nonce += 1;
    }

    // Send the batch and store its transaction hashes
    let handles = wallet
        .provider
        .send_txs_batch(signed_transfers)
        .await?
        .into_iter()
        .map(|tx_hash| SyncTransactionHandle::new(tx_hash, wallet.provider.clone()));

    for handle in handles {
        handle
            .verify_timeout(Duration::from_secs(180))
            .wait_for_verify()
            .await?;
    }

    Ok(())
}
