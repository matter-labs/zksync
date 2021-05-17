//! Integration test for zkSync Rust SDK.
//!
//! In order to pass these tests, there must be a running
//! instance of zkSync server and prover:
//!
//! ```bash
//! zk server &!
//! zk dummy-prover run &!
//! zk test integration rust-sdk
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
use std::{convert::TryFrom, env};

use num::Zero;

use zksync::operations::SyncTransactionHandle;
use zksync::{
    error::ClientError,
    ethereum::{ierc20_contract, PriorityOpHandle},
    provider::Provider,
    types::BlockStatus,
    web3::{
        contract::{Contract, Options},
        transports::Http,
        types::{Address, H160, H256, U256},
    },
    zksync_types::{
        tx::PackedEthSignature, PriorityOp, PriorityOpId, Token, TokenLike, TxFeeTypes, ZkSyncTx,
    },
    EthereumProvider, Network, RpcProvider, Wallet, WalletCredentials,
};
use zksync_eth_signer::{EthereumSigner, PrivateKeySigner};

const ETH_ADDR: &str = "36615Cf349d7F6344891B1e7CA7C72883F5dc049";
const ETH_PRIVATE_KEY: &str = "7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110";
const LOCALHOST_WEB3_ADDR: &str = "http://127.0.0.1:8545";
const DOCKER_WEB3_ADDR: &str = "http://geth:8545";

fn web3_addr() -> &'static str {
    let ci: u8 = env::var("CI").map_or(0, |s| s.parse().unwrap());
    if ci == 1 {
        DOCKER_WEB3_ADDR
    } else {
        LOCALHOST_WEB3_ADDR
    }
}

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
async fn get_ethereum_balance<S: EthereumSigner>(
    eth_provider: &EthereumProvider<S>,
    address: Address,
    token: &Token,
) -> Result<U256, anyhow::Error> {
    if token.symbol == "ETH" {
        return eth_provider
            .client()
            .eth_balance(address)
            .await
            .map_err(|_e| anyhow::anyhow!("failed to request balance from Ethereum {}", _e));
    }
    eth_provider
        .client()
        .call_contract_function(
            "balanceOf",
            address,
            None,
            Options::default(),
            None,
            token.address,
            ierc20_contract(),
        )
        .await
        .map_err(|_e| anyhow::anyhow!("failed to request erc20 balance from Ethereum"))
}

async fn wait_for_deposit_and_update_account_id<S, P>(wallet: &mut Wallet<S, P>)
where
    S: EthereumSigner,
    P: Provider + Clone,
{
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

    let provider = RpcProvider::new(Network::Localhost);
    let eth_signer = PrivateKeySigner::new(main_eth_private_key);
    let credentials =
        WalletCredentials::from_eth_signer(main_eth_address, eth_signer, Network::Localhost)
            .await
            .unwrap();

    let wallet = Wallet::new(provider, credentials).await?;
    let ethereum = wallet.ethereum(web3_addr()).await?;
    let hash = ethereum
        .transfer(token_like.into(), amount.into(), to)
        .await
        .unwrap();

    ethereum.wait_for_tx(hash).await?;
    Ok(())
}

/// Creates a new wallet and tries to make a transfer
/// from a new wallet without SigningKey.
async fn test_tx_fail<S, P>(zksync_depositor_wallet: &Wallet<S, P>) -> Result<(), anyhow::Error>
where
    S: EthereumSigner,
    P: Provider + Clone,
{
    let provider = RpcProvider::new(Network::Localhost);

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
async fn test_deposit<S, P>(
    deposit_wallet: &Wallet<S, P>,
    sync_wallet: &mut Wallet<S, P>,
    token: &Token,
    amount: u128,
) -> Result<(), anyhow::Error>
where
    S: EthereumSigner,
    P: Provider + Clone,
{
    let ethereum = deposit_wallet.ethereum(web3_addr()).await?;

    if !deposit_wallet.tokens.is_eth(token.address.into()) {
        if !ethereum.is_erc20_deposit_approved(token.address).await? {
            let tx_approve_deposits = ethereum
                .limited_approve_erc20_token_deposits(token.address, U256::from(amount))
                .await?;
            ethereum.wait_for_tx(tx_approve_deposits).await?;
        }

        assert!(
            ethereum
                .is_limited_erc20_deposit_approved(token.address, U256::from(amount))
                .await?,
            "Token should be approved"
        );
    };

    let deposit_tx_hash = ethereum
        .deposit(
            &token.symbol as &str,
            U256::from(amount),
            sync_wallet.address(),
        )
        .await?;

    ethereum.wait_for_tx(deposit_tx_hash).await?;
    wait_for_deposit_and_update_account_id(sync_wallet).await;

    if !sync_wallet.tokens.is_eth(token.address.into()) {
        // It should not be approved because we have approved only DEPOSIT_AMOUNT, not the maximum possible amount of deposit
        assert!(
            !ethereum
                .is_limited_erc20_deposit_approved(token.address, U256::from(amount))
                .await?
        );
        // Unlimited approve for deposit
        let tx_approve_deposits = ethereum.approve_erc20_token_deposits(token.address).await?;
        ethereum.wait_for_tx(tx_approve_deposits).await?;
        assert!(ethereum.is_erc20_deposit_approved(token.address).await?);
    }

    // To be sure that the deposit is committed, we need to listen to the event `NewPriorityRequest`
    // rust SDK doesn't support getting this information yet, but it will be added soon.
    // assert_eq!(balance_after - balance_before, u256_to_big_dec(amount / 2));

    Ok(())
}

/// Checks the correctness of the `ChangePubKey` operation.
async fn test_change_pubkey<S, P>(
    sync_wallet: &Wallet<S, P>,
    token_symbol: &str,
) -> Result<(), anyhow::Error>
where
    S: EthereumSigner,
    P: Provider + Clone,
{
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
async fn test_transfer<S, P>(
    alice: &Wallet<S, P>,
    bob: &Wallet<S, P>,
    token_symbol: &str,
    transfer_amount: u128,
) -> Result<(), anyhow::Error>
where
    S: EthereumSigner,
    P: Provider + Clone,
{
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
        .commit_timeout(Duration::from_secs(180))
        .wait_for_commit()
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
async fn test_transfer_to_self<S, P>(
    sync_wallet: &Wallet<S, P>,
    token_symbol: &str,
    transfer_amount: u128,
) -> Result<(), anyhow::Error>
where
    S: EthereumSigner,
    P: Provider + Clone,
{
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
        .commit_timeout(Duration::from_secs(180))
        .wait_for_commit()
        .await?;

    let balance_after = sync_wallet
        .get_balance(BlockStatus::Committed, token_symbol)
        .await?;

    assert_eq!(balance_before - balance_after, total_fee);

    Ok(())
}

/// Makes a withdraw operation on L2
/// checks the correctness of their execution.
async fn test_withdraw<S, P>(
    eth_provider: &EthereumProvider<S>,
    main_contract: &Contract<Http>,
    sync_wallet: &Wallet<S, P>,
    withdraw_to: &Wallet<S, P>,
    token: &Token,
    amount: u128,
) -> Result<(), anyhow::Error>
where
    S: EthereumSigner,
    P: Provider + Clone,
{
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
            "getPendingBalance",
            (withdraw_to.address(), token.address),
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
            "getPendingBalance",
            (withdraw_to.address(), token.address),
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
async fn move_funds<S, P>(
    main_contract: &Contract<Http>,
    eth_provider: &EthereumProvider<S>,
    depositor_wallet: &Wallet<S, P>,
    alice: &mut Wallet<S, P>,
    bob: &Wallet<S, P>,
    token_like: impl Into<TokenLike>,
    deposit_amount: u128,
) -> Result<(), anyhow::Error>
where
    S: EthereumSigner,
    P: Provider + Clone,
{
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
async fn init_account_with_one_ether(
) -> Result<Wallet<PrivateKeySigner, RpcProvider>, anyhow::Error> {
    let (eth_address, eth_private_key) = eth_random_account_credentials();

    // Transfer funds from "rich" account to a randomly created one (so we won't reuse the same
    // account in subsequent test runs).
    transfer_to("ETH", one_ether(), eth_address).await?;

    let provider = RpcProvider::new(Network::Localhost);

    let eth_signer = PrivateKeySigner::new(eth_private_key);
    let credentials =
        WalletCredentials::from_eth_signer(eth_address, eth_signer, Network::Localhost)
            .await
            .unwrap();

    let mut wallet = Wallet::new(provider, credentials).await?;
    let ethereum = wallet.ethereum(web3_addr()).await?;

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

async fn make_wallet(
    provider: RpcProvider,
    (eth_address, eth_private_key): (H160, H256),
) -> Result<Wallet<PrivateKeySigner, RpcProvider>, ClientError> {
    let eth_signer = PrivateKeySigner::new(eth_private_key);
    let credentials =
        WalletCredentials::from_eth_signer(eth_address, eth_signer, Network::Localhost).await?;
    Wallet::new(provider, credentials).await
}

#[tokio::test]
#[cfg_attr(not(feature = "integration-tests"), ignore)]
async fn comprehensive_test() -> Result<(), anyhow::Error> {
    let provider = RpcProvider::new(Network::Localhost);

    let main_wallet = make_wallet(provider.clone(), eth_main_account_credentials()).await?;
    let sync_depositor_wallet =
        make_wallet(provider.clone(), eth_random_account_credentials()).await?;
    let mut alice_wallet1 = make_wallet(provider.clone(), eth_random_account_credentials()).await?;
    let bob_wallet1 = make_wallet(provider.clone(), eth_random_account_credentials()).await?;

    let ethereum = main_wallet.ethereum(web3_addr()).await?;

    let main_contract = {
        let address_response = provider.contract_address().await?;
        let contract_address = if address_response.main_contract.starts_with("0x") {
            &address_response.main_contract[2..]
        } else {
            &address_response.main_contract
        }
        .parse()?;
        ethereum
            .client()
            .main_contract_with_address(contract_address)
    };

    let token_eth = sync_depositor_wallet
        .tokens
        .resolve("ETH".into())
        .ok_or_else(|| anyhow::anyhow!("Error resolve token"))?;
    let token_dai = sync_depositor_wallet
        .tokens
        .resolve("DAI".into())
        .ok_or_else(|| anyhow::anyhow!("Error resolve token"))?;

    let dai_deposit_amount = U256::from(10).pow(18.into()) * 10000; // 10000 DAI

    // Move ETH to wallets so they will have some funds for L1 transactions.
    let eth_deposit_amount = U256::from(10).pow(17.into()); // 0.1 ETH
    transfer_to("ETH", eth_deposit_amount, sync_depositor_wallet.address()).await?;
    transfer_to("ETH", eth_deposit_amount, alice_wallet1.address()).await?;
    transfer_to("ETH", eth_deposit_amount, bob_wallet1.address()).await?;

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
        .commit_timeout(Duration::from_secs(180))
        .wait_for_commit()
        .await?;

    Ok(())
}

#[tokio::test]
#[cfg_attr(not(feature = "integration-tests"), ignore)]
async fn nft_test() -> Result<(), anyhow::Error> {
    let alice = init_account_with_one_ether().await?;
    let bob = init_account_with_one_ether().await?;

    let alice_balance_before = alice.get_balance(BlockStatus::Committed, "ETH").await?;
    let bob_balance_before = bob.get_balance(BlockStatus::Committed, "ETH").await?;

    // Perform a mint nft transaction.
    let fee = alice
        .provider
        .get_tx_fee(TxFeeTypes::MintNFT, alice.address(), "ETH")
        .await?
        .total_fee;

    let handle = alice
        .start_mint_nft()
        .recipient(alice.signer.address)
        .content_hash(H256::zero())
        .fee_token("ETH")?
        .fee(fee.clone())
        .send()
        .await?;

    handle
        .verify_timeout(Duration::from_secs(180))
        .wait_for_verify()
        .await?;

    let nft = alice
        .account_info()
        .await?
        .verified
        .nfts
        .values()
        .last()
        .expect("NFT was not minted")
        .clone();
    let alice_balance_after_mint = alice.get_balance(BlockStatus::Committed, "ETH").await?;
    assert_eq!(fee + alice_balance_after_mint.clone(), alice_balance_before);

    // Perform a transfer nft transaction.
    let fee = alice
        .provider
        .get_txs_batch_fee(
            vec![TxFeeTypes::Transfer, TxFeeTypes::Transfer],
            vec![bob.address(), bob.address()],
            "ETH",
        )
        .await?;
    let handles = alice
        .start_transfer_nft()
        .to(bob.signer.address)
        .nft(nft.clone())
        .fee_token("ETH")?
        .fee(fee.clone())
        .send()
        .await?;

    for handle in handles {
        handle
            .commit_timeout(Duration::from_secs(180))
            .wait_for_commit()
            .await?;
    }

    let alice_balance_after_transfer = alice.get_balance(BlockStatus::Committed, "ETH").await?;
    let alice_nft_balance = alice.get_nft(BlockStatus::Committed, nft.id).await?;
    let bob_nft_balance = bob.get_nft(BlockStatus::Committed, nft.id).await?;
    assert_eq!(fee + alice_balance_after_transfer, alice_balance_after_mint);
    assert!(alice_nft_balance.is_none());
    assert!(bob_nft_balance.is_some());

    //Perform a withdraw nft transaction.
    let fee = alice
        .provider
        .get_tx_fee(TxFeeTypes::WithdrawNFT, bob.address(), "ETH")
        .await?
        .total_fee;

    let handle = bob
        .start_withdraw_nft()
        .to(bob.signer.address)
        .token(nft.id)?
        .fee_token("ETH")?
        .fee(fee.clone())
        .send()
        .await?;

    handle
        .commit_timeout(Duration::from_secs(180))
        .wait_for_commit()
        .await?;
    let bob_balance_after_withdraw = bob.get_balance(BlockStatus::Committed, "ETH").await?;
    let bob_nft_balance = bob.get_nft(BlockStatus::Committed, nft.id).await?;
    assert_eq!(fee + bob_balance_after_withdraw, bob_balance_before);
    assert!(bob_nft_balance.is_none());

    Ok(())
}

#[tokio::test]
#[cfg_attr(not(feature = "integration-tests"), ignore)]
async fn full_exit_test() -> Result<(), anyhow::Error> {
    let wallet = init_account_with_one_ether().await?;
    let ethereum = wallet.ethereum(web3_addr()).await?;

    // Mint NFT
    let handle = wallet
        .start_mint_nft()
        .recipient(wallet.signer.address)
        .content_hash(H256::zero())
        .fee_token("ETH")?
        .send()
        .await?;

    handle
        .verify_timeout(Duration::from_secs(180))
        .wait_for_verify()
        .await?;

    // ETH full exit
    let full_exit_tx_hash = ethereum
        .full_exit("ETH", wallet.account_id().unwrap())
        .await?;
    let receipt = ethereum.wait_for_tx(full_exit_tx_hash).await?;
    let mut serial_id = None;
    for log in receipt.logs {
        if let Ok(op) = PriorityOp::try_from(log) {
            serial_id = Some(op.serial_id);
        }
    }
    let handle = PriorityOpHandle::new(PriorityOpId(serial_id.unwrap()), wallet.provider.clone());
    handle
        .commit_timeout(Duration::from_secs(180))
        .wait_for_commit()
        .await?;

    let balance = wallet.get_balance(BlockStatus::Committed, "ETH").await?;
    assert!(balance.is_zero());

    // NFT full exit
    let token_id = wallet
        .account_info()
        .await?
        .verified
        .nfts
        .values()
        .last()
        .expect("NFT was not minted")
        .id;
    let full_exit_nft_tx_hash = ethereum
        .full_exit_nft(token_id, wallet.account_id().unwrap())
        .await?;
    let receipt = ethereum.wait_for_tx(full_exit_nft_tx_hash).await?;
    let mut serial_id = None;
    for log in receipt.logs {
        if let Ok(op) = PriorityOp::try_from(log) {
            serial_id = Some(op.serial_id);
        }
    }
    let handle = PriorityOpHandle::new(PriorityOpId(serial_id.unwrap()), wallet.provider.clone());
    handle
        .commit_timeout(Duration::from_secs(180))
        .wait_for_commit()
        .await?;

    let nft_balance = wallet.get_nft(BlockStatus::Committed, token_id).await?;
    assert!(nft_balance.is_none());

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

    // Obtain total fee for this batch
    let mut total_fee = Some(
        wallet
            .provider
            .get_txs_batch_fee(
                vec![TxFeeTypes::Transfer; recipients.len()],
                recipients.clone(),
                token_like.clone(),
            )
            .await?,
    );

    for recipient in recipients {
        let (transfer, signature) = wallet
            .signer
            .sign_transfer(
                token.clone(),
                1_000_000u64.into(),
                // Set a total batch fee in the first transaction.
                total_fee.take().unwrap_or_default(),
                recipient,
                nonce,
                Default::default(),
            )
            .await
            .expect("Transfer signing error");

        signed_transfers.push((ZkSyncTx::Transfer(Box::new(transfer)), signature));

        *nonce += 1;
    }

    // Send the batch and store its transaction hashes
    let handles = wallet
        .provider
        .send_txs_batch(signed_transfers, None)
        .await?
        .into_iter()
        .map(|tx_hash| SyncTransactionHandle::new(tx_hash, wallet.provider.clone()));

    for handle in handles {
        handle
            .commit_timeout(Duration::from_secs(180))
            .wait_for_commit()
            .await?;
    }

    Ok(())
}
