use super::{CommitRequest, ZkSyncStateInitParams, ZkSyncStateKeeper};
use crate::mempool::ProposedBlock;
use futures::{channel::mpsc, stream::StreamExt};
use num::BigUint;
use zksync_crypto::{
    priv_key_from_fs,
    rand::{Rng, SeedableRng, XorShiftRng},
    PrivateKey,
};
use zksync_types::tx::TimeRange;
use zksync_types::{
    mempool::SignedTxVariant, mempool::SignedTxsBatch, tx::PackedEthSignature, AccountId, H160, *,
};

struct StateKeeperTester {
    state_keeper: ZkSyncStateKeeper,
    response_rx: mpsc::Receiver<CommitRequest>,
    fee_collector: AccountId,
}

impl StateKeeperTester {
    fn new(available_chunk_size: usize, max_iterations: usize, fast_iterations: usize) -> Self {
        const CHANNEL_SIZE: usize = 32768;
        let (_request_tx, request_rx) = mpsc::channel(CHANNEL_SIZE);
        let (response_tx, response_rx) = mpsc::channel(CHANNEL_SIZE);

        let fee_collector = Account::default_with_address(&H160::random());

        let mut init_params = ZkSyncStateInitParams::default();
        init_params.insert_account(AccountId(0), fee_collector.clone());

        let state_keeper = ZkSyncStateKeeper::new(
            init_params,
            fee_collector.address,
            request_rx,
            response_tx,
            vec![available_chunk_size],
            max_iterations,
            fast_iterations,
            None,
        );

        Self {
            state_keeper,
            response_rx,
            fee_collector: AccountId(0),
        }
    }

    fn set_balance(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        amount: impl Into<BigUint>,
    ) {
        let mut account = self
            .state_keeper
            .state
            .get_account(account_id)
            .expect("account doesn't exist");

        account.set_balance(token_id, amount.into());

        self.state_keeper.state.insert_account(account_id, account);
    }

    fn add_account(&mut self, account_id: AccountId) -> (Account, PrivateKey) {
        let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);
        let sk = priv_key_from_fs(rng.gen());
        let eth_sk = H256::random();
        let address = PackedEthSignature::address_from_private_key(&eth_sk)
            .expect("Can't get address from the ETH secret key");

        let mut account = Account::default_with_address(&address);
        account.pub_key_hash = PubKeyHash::from_privkey(&sk);
        self.state_keeper
            .state
            .insert_account(account_id, account.clone());
        (account, sk)
    }
}

fn create_account_and_transfer<B: Into<BigUint>>(
    tester: &mut StateKeeperTester,
    token_id: TokenId,
    account_id: AccountId,
    balance: B,
    transfer_amount: B,
) -> SignedZkSyncTx {
    let (account, sk) = tester.add_account(account_id);
    tester.set_balance(account_id, token_id, balance);

    let transfer = Transfer::new_signed(
        account_id,
        account.address,
        account.address,
        token_id,
        transfer_amount.into(),
        BigUint::from(1u32),
        account.nonce,
        Default::default(),
        &sk,
    )
    .unwrap();
    SignedZkSyncTx {
        tx: ZkSyncTx::Transfer(Box::new(transfer)),
        eth_sign_data: None,
    }
}

fn create_account_and_withdrawal<B: Into<BigUint>>(
    tester: &mut StateKeeperTester,
    token_id: TokenId,
    account_id: AccountId,
    balance: B,
    withdraw_amount: B,
    time_range: TimeRange,
) -> SignedZkSyncTx {
    create_account_and_withdrawal_impl(
        tester,
        token_id,
        account_id,
        balance,
        withdraw_amount,
        false,
        time_range,
    )
}

fn create_account_and_fast_withdrawal<B: Into<BigUint>>(
    tester: &mut StateKeeperTester,
    token_id: TokenId,
    account_id: AccountId,
    balance: B,
    withdraw_amount: B,
    time_range: TimeRange,
) -> SignedZkSyncTx {
    create_account_and_withdrawal_impl(
        tester,
        token_id,
        account_id,
        balance,
        withdraw_amount,
        true,
        time_range,
    )
}

#[allow(clippy::clippy::too_many_arguments)]
fn create_account_and_withdrawal_impl<B: Into<BigUint>>(
    tester: &mut StateKeeperTester,
    token_id: TokenId,
    account_id: AccountId,
    balance: B,
    withdraw_amount: B,
    fast: bool,
    time_range: TimeRange,
) -> SignedZkSyncTx {
    let (account, sk) = tester.add_account(account_id);
    tester.set_balance(account_id, token_id, balance);

    let mut withdraw = Withdraw::new_signed(
        account_id,
        account.address,
        account.address,
        token_id,
        withdraw_amount.into(),
        BigUint::from(1u32),
        account.nonce,
        time_range,
        &sk,
    )
    .unwrap();

    withdraw.fast = fast;

    SignedZkSyncTx {
        tx: ZkSyncTx::Withdraw(Box::new(withdraw)),
        eth_sign_data: None,
    }
}

pub fn create_deposit(token: TokenId, amount: impl Into<BigUint>) -> PriorityOp {
    let address = H160::random();
    let deposit = Deposit {
        from: address,
        to: address,
        amount: amount.into(),
        token,
    };
    PriorityOp {
        data: ZkSyncPriorityOp::Deposit(deposit),
        serial_id: 0,
        deadline_block: 0,
        eth_hash: H256::zero(),
        eth_block: 0,
    }
}

async fn apply_single_transfer(tester: &mut StateKeeperTester) {
    let transfer = create_account_and_transfer(tester, TokenId(0), AccountId(1), 200u32, 100u32);
    let proposed_block = ProposedBlock {
        txs: vec![SignedTxVariant::Tx(transfer)],
        priority_ops: Vec::new(),
    };
    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;
}

async fn apply_batch_with_two_transfers(tester: &mut StateKeeperTester) {
    let first_transfer =
        create_account_and_transfer(tester, TokenId(0), AccountId(1), 200u32, 100u32);
    let second_transfer =
        create_account_and_transfer(tester, TokenId(0), AccountId(2), 200u32, 100u32);
    let proposed_block = ProposedBlock {
        txs: vec![SignedTxVariant::Batch(SignedTxsBatch {
            txs: vec![first_transfer, second_transfer],
            batch_id: 1,
            eth_signatures: Vec::new(),
        })],
        priority_ops: Vec::new(),
    };
    tester
        .state_keeper
        .execute_proposed_block(proposed_block)
        .await;
}

/// Checks that StateKeeper will panic with incorrect initialization data
#[test]
#[should_panic]
fn test_create_incorrect_state_keeper() {
    const CHANNEL_SIZE: usize = 32768;
    const MAX_ITERATIONS: usize = 100;
    const FAST_ITERATIONS: usize = 100;

    let (_request_tx, request_rx) = mpsc::channel(CHANNEL_SIZE);
    let (response_tx, _response_rx) = mpsc::channel(CHANNEL_SIZE);

    let fee_collector = Account::default_with_address(&H160::random());

    let mut init_params = ZkSyncStateInitParams::default();
    init_params.insert_account(AccountId(0), fee_collector.clone());

    // should panic
    ZkSyncStateKeeper::new(
        init_params,
        fee_collector.address,
        request_rx,
        response_tx,
        vec![1, 2, 2], // `available_block_chunk_sizes` must be strictly increasing.
        MAX_ITERATIONS,
        FAST_ITERATIONS,
        None,
    );
}

mod apply_priority_op {
    use super::*;

    /// Checks if deposit is processed correctly by the state_keeper
    #[test]
    fn success() {
        let mut tester = StateKeeperTester::new(8, 1, 1);
        let old_pending_block = tester.state_keeper.pending_block.clone();
        let deposit = create_deposit(TokenId(0), 145u32);
        let result = tester.state_keeper.apply_priority_op(deposit);
        let pending_block = tester.state_keeper.pending_block;
        assert!(result.is_ok());
        assert!(pending_block.chunks_left < old_pending_block.chunks_left);
        assert_eq!(
            pending_block.pending_op_block_index,
            old_pending_block.pending_op_block_index + 1
        );
        assert!(!pending_block.account_updates.is_empty());
        assert!(!pending_block.success_operations.is_empty());
        assert_eq!(tester.state_keeper.current_unprocessed_priority_op, 1);
    }

    /// Checks if processing deposit fails because of
    /// small number of chunks left in the block
    #[test]
    fn not_enough_chunks() {
        let mut tester = StateKeeperTester::new(1, 1, 1);
        let deposit = create_deposit(TokenId(0), 1u32);
        let result = tester.state_keeper.apply_priority_op(deposit);
        assert!(result.is_err());
    }
}

mod apply_tx {
    use super::*;
    use zksync_types::gas_counter::{VerifyCost, TX_GAS_LIMIT};

    /// Checks if withdrawal is processed correctly by the state_keeper
    #[test]
    fn success() {
        let mut tester = StateKeeperTester::new(6, 1, 1);
        let old_pending_block = tester.state_keeper.pending_block.clone();
        let withdraw = create_account_and_withdrawal(
            &mut tester,
            TokenId(0),
            AccountId(1),
            200u32,
            145u32,
            Default::default(),
        );
        let result = tester.state_keeper.apply_tx(&withdraw);
        let pending_block = tester.state_keeper.pending_block;

        assert!(result.is_ok());
        assert!(pending_block.chunks_left < old_pending_block.chunks_left);
        assert_eq!(
            pending_block.pending_op_block_index,
            old_pending_block.pending_op_block_index + 1
        );
        assert!(!pending_block.account_updates.is_empty());
        assert!(!pending_block.success_operations.is_empty());
        assert!(!pending_block.collected_fees.is_empty());
    }

    /// Checks if fast withdrawal makes fast processing required
    #[test]
    fn fast_withdrawal() {
        let mut tester = StateKeeperTester::new(6, 1, 1);
        let old_pending_block = tester.state_keeper.pending_block.clone();
        let withdraw = create_account_and_fast_withdrawal(
            &mut tester,
            TokenId(0),
            AccountId(1),
            200u32,
            145u32,
            Default::default(),
        );
        let result = tester.state_keeper.apply_tx(&withdraw);
        let pending_block = tester.state_keeper.pending_block;

        assert!(result.is_ok());
        assert_eq!(old_pending_block.fast_processing_required, false);
        assert_eq!(pending_block.fast_processing_required, true);
    }

    /// Checks if withdrawal that will fail is processed correctly
    #[test]
    fn failure() {
        let mut tester = StateKeeperTester::new(6, 1, 1);
        let old_pending_block = tester.state_keeper.pending_block.clone();
        let withdraw = create_account_and_withdrawal(
            &mut tester,
            TokenId(0),
            AccountId(1),
            100u32,
            145u32,
            Default::default(),
        );
        let result = tester.state_keeper.apply_tx(&withdraw);
        let pending_block = tester.state_keeper.pending_block;

        assert!(result.is_ok());
        assert_eq!(pending_block.chunks_left, old_pending_block.chunks_left);
        assert_eq!(
            pending_block.pending_op_block_index,
            old_pending_block.pending_op_block_index
        );
        assert!(pending_block.account_updates.is_empty());
        assert!(!pending_block.failed_txs.is_empty());
        assert!(pending_block.collected_fees.is_empty());
    }

    /// Checks if processing withdrawal fails because of
    /// small number of chunks left in the block
    #[test]
    fn not_enough_chunks() {
        let mut tester = StateKeeperTester::new(1, 1, 1);
        let withdraw = create_account_and_withdrawal(
            &mut tester,
            TokenId(0),
            AccountId(1),
            200u32,
            145u32,
            Default::default(),
        );
        let result = tester.state_keeper.apply_tx(&withdraw);
        assert!(result.is_err());
    }

    /// Checks if processing withdrawal fails because the gas limit is reached.
    /// This sends 46 withdrawals (very inefficient, but all constants in
    /// GasCounter are hardcoded, so I see no way out)
    #[test]
    fn gas_limit_reached() {
        let withdrawals_number = (TX_GAS_LIMIT - VerifyCost::base_cost().as_u64() * 130 / 100)
            / (VerifyCost::WITHDRAW_COST * 130 / 100);
        let mut tester = StateKeeperTester::new(6 * withdrawals_number as usize, 1, 1);
        for i in 1..=withdrawals_number {
            let withdrawal = create_account_and_withdrawal(
                &mut tester,
                TokenId(0),
                AccountId(i as u32),
                200u32,
                145u32,
                Default::default(),
            );
            let result = tester.state_keeper.apply_tx(&withdrawal);
            if i <= withdrawals_number {
                assert!(
                    result.is_ok(),
                    "i: {}, withdrawals: {}",
                    i,
                    withdrawals_number
                )
            } else {
                assert!(
                    result.is_err(),
                    "i: {}, withdrawals: {}",
                    i,
                    withdrawals_number
                )
            }
        }
    }
}

/// Checks if block sealing is done correctly by sealing a block
/// with 1 priority_op, 1 succeeded tx, 1 failed tx
#[tokio::test]
async fn seal_pending_block() {
    let mut tester = StateKeeperTester::new(20, 3, 3);
    let good_withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(0),
        AccountId(1),
        200u32,
        145u32,
        Default::default(),
    );
    let bad_withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(2),
        AccountId(2),
        100u32,
        145u32,
        Default::default(),
    );
    let deposit = create_deposit(TokenId(0), 12u32);

    assert!(tester.state_keeper.apply_tx(&good_withdraw).is_ok());
    assert!(tester.state_keeper.apply_tx(&bad_withdraw).is_ok());
    assert!(tester.state_keeper.apply_priority_op(deposit).is_ok());

    let old_updates_len = tester.state_keeper.pending_block.account_updates.len();
    tester.state_keeper.seal_pending_block().await;

    assert!(tester.state_keeper.pending_block.failed_txs.is_empty());
    assert!(tester
        .state_keeper
        .pending_block
        .success_operations
        .is_empty());
    assert!(tester.state_keeper.pending_block.collected_fees.is_empty());
    assert!(tester.state_keeper.pending_block.account_updates.is_empty());
    assert_eq!(tester.state_keeper.pending_block.chunks_left, 20);

    if let Some(CommitRequest::Block((block, updates))) = tester.response_rx.next().await {
        let collected_fees = tester
            .state_keeper
            .state
            .get_account(tester.fee_collector)
            .unwrap()
            .get_balance(TokenId(0));
        assert_eq!(block.block.block_transactions.len(), 3);
        assert_eq!(collected_fees, BigUint::from(1u32));
        assert_eq!(block.block.processed_priority_ops, (0, 1));
        assert_eq!(
            tester.state_keeper.state.block_number,
            block.block.block_number + 1
        );
        assert_eq!(
            updates.account_updates.len(),
            // + 1 here is for the update corresponding to collected fee
            old_updates_len - updates.first_update_order_id + 1
        );
    } else {
        panic!("Block is not received!");
    }
}

/// Checks if block storing is done correctly by storing a block
/// with 1 priority_op, 1 succeeded tx, 1 failed tx
#[tokio::test]
async fn store_pending_block() {
    let mut tester = StateKeeperTester::new(20, 3, 3);
    let good_withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(0),
        AccountId(1),
        200u32,
        145u32,
        Default::default(),
    );
    let bad_withdraw = create_account_and_withdrawal(
        &mut tester,
        TokenId(2),
        AccountId(2),
        100u32,
        145u32,
        Default::default(),
    );
    let deposit = create_deposit(TokenId(0), 12u32);

    assert!(tester.state_keeper.apply_tx(&good_withdraw).is_ok());
    assert!(tester.state_keeper.apply_tx(&bad_withdraw).is_ok());
    assert!(tester.state_keeper.apply_priority_op(deposit).is_ok());

    tester.state_keeper.store_pending_block().await;

    if let Some(CommitRequest::PendingBlock((block, _))) = tester.response_rx.next().await {
        assert_eq!(block.number, tester.state_keeper.state.block_number);
        assert_eq!(
            block.chunks_left,
            tester.state_keeper.pending_block.chunks_left
        );
        assert_eq!(
            block.unprocessed_priority_op_before,
            tester
                .state_keeper
                .pending_block
                .unprocessed_priority_op_before
        );
        assert_eq!(
            block.pending_block_iteration,
            tester.state_keeper.pending_block.pending_block_iteration
        );
        assert_eq!(
            block.success_operations.len(),
            tester.state_keeper.pending_block.success_operations.len()
        );
        assert_eq!(
            block.failed_txs.len(),
            tester.state_keeper.pending_block.failed_txs.len()
        );
    } else {
        panic!("Block is not received!");
    }
}

mod execute_proposed_block {

    use super::*;
    use zksync_types::gas_counter::GasCounter;

    /// Checks if executing a proposed_block with just enough chunks is done correctly
    /// and checks if number of chunks left is correct after each operation
    #[tokio::test]
    async fn just_enough_chunks() {
        let mut tester = StateKeeperTester::new(8, 3, 3);

        // First batch
        apply_batch_with_two_transfers(&mut tester).await;
        if let Some(CommitRequest::PendingBlock((block, _))) = tester.response_rx.next().await {
            assert_eq!(block.chunks_left, 4);
        } else {
            panic!("Block is not received!");
        }

        // Second batch
        apply_batch_with_two_transfers(&mut tester).await;

        // Check sealed block
        if let Some(CommitRequest::Block((block, _))) = tester.response_rx.next().await {
            assert_eq!(block.block.block_transactions.len(), 4);
        } else {
            panic!("Block is not received!");
        }
    }

    /// Checks if executing a proposed_block is done correctly
    /// when two batches don`t fit into one block.
    /// Also, checks if number of chunks left is correct after each operation
    #[tokio::test]
    async fn chunks_to_fit_three_transfers_2_2_1() {
        let mut tester = StateKeeperTester::new(6, 3, 3);

        // First batch
        apply_batch_with_two_transfers(&mut tester).await;
        if let Some(CommitRequest::PendingBlock((block, _))) = tester.response_rx.next().await {
            assert_eq!(block.chunks_left, 2);
        } else {
            panic!("Block is not received!");
        }

        // Second batch
        apply_batch_with_two_transfers(&mut tester).await;
        if let Some(CommitRequest::Block((block, _))) = tester.response_rx.next().await {
            assert_eq!(block.block.block_transactions.len(), 2);
        } else {
            panic!("Block is not received!");
        }
        if let Some(CommitRequest::PendingBlock((block, _))) = tester.response_rx.next().await {
            assert_eq!(block.chunks_left, 2);
        } else {
            panic!("Block is not received!");
        }

        // Single tx
        apply_single_transfer(&mut tester).await;

        // Check sealed block
        if let Some(CommitRequest::Block((block, _))) = tester.response_rx.next().await {
            assert_eq!(block.block.block_transactions.len(), 3);
        } else {
            panic!("Block is not received!");
        }
    }

    /// Checks if executing a proposed_block is done correctly
    /// when two single txs and one batch don`t fit into one block.
    /// Also, checks if number of chunks left is correct after each operation
    #[tokio::test]
    async fn chunks_to_fit_three_transfers_1_1_2_1() {
        let mut tester = StateKeeperTester::new(6, 3, 3);

        // First single tx
        apply_single_transfer(&mut tester).await;
        if let Some(CommitRequest::PendingBlock((block, _))) = tester.response_rx.next().await {
            assert_eq!(block.chunks_left, 4);
        } else {
            panic!("Block is not received!");
        }

        // Second single tx
        apply_single_transfer(&mut tester).await;
        if let Some(CommitRequest::PendingBlock((block, _))) = tester.response_rx.next().await {
            assert_eq!(block.chunks_left, 2);
        } else {
            panic!("Block is not received!");
        }

        // First batch
        apply_batch_with_two_transfers(&mut tester).await;
        if let Some(CommitRequest::Block((block, _))) = tester.response_rx.next().await {
            assert_eq!(block.block.block_transactions.len(), 2);
        } else {
            panic!("Block is not received!");
        }
        if let Some(CommitRequest::PendingBlock((block, _))) = tester.response_rx.next().await {
            assert_eq!(block.chunks_left, 2);
        } else {
            panic!("Block is not received!");
        }

        // Last single tx
        apply_single_transfer(&mut tester).await;

        // Check sealed block
        if let Some(CommitRequest::Block((block, _))) = tester.response_rx.next().await {
            assert_eq!(block.block.block_transactions.len(), 3);
        } else {
            panic!("Block is not received!");
        }
    }

    /// Checks if executing a small proposed_block is done correctly
    #[tokio::test]
    async fn small() {
        let mut tester = StateKeeperTester::new(20, 3, 3);
        let good_withdraw = create_account_and_withdrawal(
            &mut tester,
            TokenId(0),
            AccountId(1),
            200u32,
            145u32,
            Default::default(),
        );
        let bad_withdraw = create_account_and_withdrawal(
            &mut tester,
            TokenId(2),
            AccountId(2),
            100u32,
            145u32,
            Default::default(),
        );
        let deposit = create_deposit(TokenId(0), 12u32);
        let proposed_block = ProposedBlock {
            txs: vec![
                SignedTxVariant::Tx(good_withdraw),
                SignedTxVariant::Tx(bad_withdraw),
            ],
            priority_ops: vec![deposit],
        };
        let pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
        tester
            .state_keeper
            .execute_proposed_block(proposed_block)
            .await;
        if let Some(CommitRequest::PendingBlock(_)) = tester.response_rx.next().await {
            assert_eq!(
                pending_block_iteration + 1,
                tester.state_keeper.pending_block.pending_block_iteration
            );
        } else {
            panic!("Block not stored");
        }
    }

    /// Checks if executing a proposed_block is done correctly
    /// There are more chunks than one can fit in 1 block,
    /// so 1 block should get sealed in the process
    #[tokio::test]
    async fn few_chunks() {
        let mut tester = StateKeeperTester::new(12, 3, 3);
        let good_withdraw = create_account_and_withdrawal(
            &mut tester,
            TokenId(0),
            AccountId(1),
            200u32,
            145u32,
            Default::default(),
        );
        let bad_withdraw = create_account_and_withdrawal(
            &mut tester,
            TokenId(2),
            AccountId(2),
            100u32,
            145u32,
            Default::default(),
        );
        let deposit = create_deposit(TokenId(0), 12u32);
        let proposed_block = ProposedBlock {
            txs: vec![
                SignedTxVariant::Tx(good_withdraw),
                SignedTxVariant::Tx(bad_withdraw),
            ],
            priority_ops: vec![deposit],
        };
        tester
            .state_keeper
            .execute_proposed_block(proposed_block)
            .await;
        assert!(matches!(
            tester.response_rx.next().await,
            Some(CommitRequest::Block(_))
        ));
        assert!(matches!(
            tester.response_rx.next().await,
            Some(CommitRequest::PendingBlock(_))
        ));
    }

    /// Checks if executing a proposed_block is done correctly
    /// max_iterations == 0, so the block should get sealed, not stored
    #[tokio::test]
    async fn few_iterations() {
        let mut tester = StateKeeperTester::new(20, 0, 0);
        let good_withdraw = create_account_and_withdrawal(
            &mut tester,
            TokenId(0),
            AccountId(1),
            200u32,
            145u32,
            Default::default(),
        );
        let bad_withdraw = create_account_and_withdrawal(
            &mut tester,
            TokenId(2),
            AccountId(2),
            100u32,
            145u32,
            Default::default(),
        );
        let deposit = create_deposit(TokenId(0), 12u32);
        let proposed_block = ProposedBlock {
            txs: vec![
                SignedTxVariant::Tx(good_withdraw),
                SignedTxVariant::Tx(bad_withdraw),
            ],
            priority_ops: vec![deposit],
        };
        tester
            .state_keeper
            .execute_proposed_block(proposed_block)
            .await;
        assert!(matches!(
            tester.response_rx.next().await,
            Some(CommitRequest::Block(_))
        ));
    }

    /// Checks that fast withdrawal causes block to be sealed faster.
    #[tokio::test]
    async fn fast_withdrawal() {
        const MAX_ITERATIONS: usize = 100;
        const FAST_ITERATIONS: usize = 0; // Seal block right after fast withdrawal.

        let mut tester = StateKeeperTester::new(6, MAX_ITERATIONS, FAST_ITERATIONS);
        let withdraw = create_account_and_fast_withdrawal(
            &mut tester,
            TokenId(0),
            AccountId(1),
            200u32,
            145u32,
            Default::default(),
        );

        let proposed_block = ProposedBlock {
            priority_ops: Vec::new(),
            txs: vec![withdraw.into()],
        };

        tester
            .state_keeper
            .execute_proposed_block(proposed_block)
            .await;

        // We should receive the next block, since it must be sealed right after.
        assert!(matches!(
            tester.response_rx.next().await,
            Some(CommitRequest::Block(_))
        ));
    }

    /// Checks the following things:
    /// 1. if proposed block is empty, no pending block is yielded from the state keeper.
    /// 2. if there were no successful operations in the block, pending block iteration is not incremented after empty or rejected-only updates.
    /// 3. if there were successful operations in the block, pending block iteration is incremented after each `execute_proposed_block` call.
    #[tokio::test]
    async fn pending_block_updates() {
        let mut tester = StateKeeperTester::new(20, 5, 5);

        // --- Phase 1: Empty pending block, empty update. ---

        // Check that empty update with empty pending block doesn't increment the iteration.
        let proposed_block = ProposedBlock {
            txs: vec![],
            priority_ops: vec![],
        };

        tester
            .state_keeper
            .execute_proposed_block(proposed_block)
            .await;

        // There should be no pending block yielded.
        let next_block = tester.response_rx.try_next();
        assert!(next_block.is_err(), "Empty pending block was yielded");

        // No successful operations in the pending block => no increment.
        let pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
        assert_eq!(pending_block_iteration, 0);

        // --- Phase 2: Empty pending block, only failed tx in update. ---

        // Then send the block with the bad transaction only
        let bad_withdraw = create_account_and_withdrawal(
            &mut tester,
            TokenId(2),
            AccountId(1),
            100u32,
            145u32,
            Default::default(),
        );
        let proposed_block = ProposedBlock {
            txs: vec![SignedTxVariant::Tx(bad_withdraw)],
            priority_ops: vec![],
        };

        tester
            .state_keeper
            .execute_proposed_block(proposed_block)
            .await;

        // Pending block should be created.
        let next_block = tester.response_rx.next().await;
        assert!(next_block.is_some(), "No pending block was yielded");

        // Iteration should still not be incremented.
        let pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
        assert_eq!(pending_block_iteration, 0);

        // --- Phase 3: Empty pending block, successful tx in update. ---

        // First, create some block with successfull operation.
        let good_withdraw = create_account_and_withdrawal(
            &mut tester,
            TokenId(2),
            AccountId(1),
            200u32,
            145u32,
            Default::default(),
        );
        let proposed_block = ProposedBlock {
            txs: vec![SignedTxVariant::Tx(good_withdraw)],
            priority_ops: vec![],
        };

        let pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
        tester
            .state_keeper
            .execute_proposed_block(proposed_block)
            .await;

        // Pending block should be created.
        let next_block = tester.response_rx.next().await;
        assert!(next_block.is_some(), "No pending block was yielded");

        // Iteration should be incremented.
        let new_pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
        assert_eq!(new_pending_block_iteration, pending_block_iteration + 1);

        // --- Phase 4: Successful tx in pending block, failed tx in update. ---

        // Then send the block with the bad transaction only
        let bad_withdraw = create_account_and_withdrawal(
            &mut tester,
            TokenId(2),
            AccountId(1),
            100u32,
            145u32,
            Default::default(),
        );
        let proposed_block = ProposedBlock {
            txs: vec![SignedTxVariant::Tx(bad_withdraw)],
            priority_ops: vec![],
        };

        let pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
        tester
            .state_keeper
            .execute_proposed_block(proposed_block)
            .await;

        // Pending block should be created.
        let next_block = tester.response_rx.next().await;
        assert!(next_block.is_some(), "No pending block was yielded");

        // Iteration should still be incremented.
        let new_pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
        assert_eq!(new_pending_block_iteration, pending_block_iteration + 1);

        // --- Phase 5: Successful tx in pending block, empty update. ---

        // Finally, execute an empty block.
        let proposed_block = ProposedBlock {
            txs: vec![],
            priority_ops: vec![],
        };

        let pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
        tester
            .state_keeper
            .execute_proposed_block(proposed_block)
            .await;

        // There should be no pending block yielded.
        let next_block = tester.response_rx.try_next();
        assert!(next_block.is_err(), "Empty pending block was yielded");

        // Iteration should still be incremented even after an empty block: there was a successful operation earlier.
        let new_pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
        assert_eq!(new_pending_block_iteration, pending_block_iteration + 1);
    }

    /// Checks that only the difference between two states of a pending block is transmitted
    /// to the committer.
    #[tokio::test]
    async fn pending_block_diff() {
        let mut tester = StateKeeperTester::new(20, 5, 5);

        let good_withdraw_1 = create_account_and_withdrawal(
            &mut tester,
            TokenId(0),
            AccountId(1),
            200u32,
            145u32,
            Default::default(),
        );
        let bad_withdraw_1 = create_account_and_withdrawal(
            &mut tester,
            TokenId(2),
            AccountId(2),
            100u32,
            145u32,
            Default::default(),
        );
        let proposed_block_1 = ProposedBlock {
            txs: vec![
                SignedTxVariant::Tx(good_withdraw_1.clone()),
                SignedTxVariant::Tx(bad_withdraw_1.clone()),
            ],
            priority_ops: vec![],
        };

        let good_withdraw_2 = create_account_and_withdrawal(
            &mut tester,
            TokenId(0),
            AccountId(3),
            200u32,
            145u32,
            Default::default(),
        );
        let bad_withdraw_2 = create_account_and_withdrawal(
            &mut tester,
            TokenId(2),
            AccountId(4),
            100u32,
            145u32,
            Default::default(),
        );
        let proposed_block_2 = ProposedBlock {
            txs: vec![
                SignedTxVariant::Tx(good_withdraw_2.clone()),
                SignedTxVariant::Tx(bad_withdraw_2.clone()),
            ],
            priority_ops: vec![],
        };

        tester
            .state_keeper
            .execute_proposed_block(proposed_block_1)
            .await;
        if let Some(CommitRequest::PendingBlock((block, _))) = tester.response_rx.next().await {
            assert_eq!(*block.number, 1); // It's the first block.
            assert_eq!(block.success_operations.len(), 1);
            assert_eq!(
                block.success_operations[0]
                    .get_executed_tx()
                    .unwrap()
                    .signed_tx
                    .hash(),
                good_withdraw_1.hash()
            );

            assert_eq!(block.failed_txs.len(), 1);
            assert_eq!(block.failed_txs[0].signed_tx.hash(), bad_withdraw_1.hash());
        } else {
            panic!("Block #1 not stored");
        }

        // Now we execute the next proposed block and expect that only the diff between `pending_block_2` and
        // `pending_block_1` will be sent.
        tester
            .state_keeper
            .execute_proposed_block(proposed_block_2)
            .await;
        if let Some(CommitRequest::PendingBlock((block, _))) = tester.response_rx.next().await {
            assert_eq!(*block.number, 1); // It still should be the first block.
            assert_eq!(block.success_operations.len(), 1);
            assert_eq!(
                block.success_operations[0]
                    .get_executed_tx()
                    .unwrap()
                    .signed_tx
                    .hash(),
                good_withdraw_2.hash()
            );

            assert_eq!(block.failed_txs.len(), 1);
            assert_eq!(block.failed_txs[0].signed_tx.hash(), bad_withdraw_2.hash());
        } else {
            panic!("Block #2 not stored");
        }
    }

    /// Checks that a transaction with a valid timestamp accepted by the statekeeper
    /// and transaction with an invalid timestamp failed.
    #[tokio::test]
    async fn transfers_with_different_timestamps() {
        let mut tester = StateKeeperTester::new(20, 5, 5);

        let token_id = TokenId(0);
        let account_from_id = AccountId(1);
        let account_to_id = AccountId(2);
        let balance = 999u32;
        let fee = 0u32;
        let (account_from, sk_from) = tester.add_account(account_from_id);
        let (account_to, _sk_to) = tester.add_account(account_to_id);
        tester.set_balance(account_from_id, token_id, balance);

        let correct_transfer = Transfer::new_signed(
            account_from_id,
            account_from.address,
            account_to.address,
            token_id,
            balance.into(),
            fee.into(),
            Nonce(0),
            Default::default(),
            &sk_from,
        )
        .unwrap();

        let mut premature_transfer = correct_transfer.clone();
        if let Some(time_range) = premature_transfer.time_range.as_mut() {
            time_range.valid_from = u64::max_value();
        }

        let mut belated_transfer = correct_transfer.clone();
        if let Some(time_range) = belated_transfer.time_range.as_mut() {
            time_range.valid_until = 0;
        }

        let correct_transfer = SignedZkSyncTx {
            tx: ZkSyncTx::Transfer(Box::new(correct_transfer)),
            eth_sign_data: None,
        };
        let premature_transfer = SignedZkSyncTx {
            tx: ZkSyncTx::Transfer(Box::new(premature_transfer)),
            eth_sign_data: None,
        };
        let belated_transfer = SignedZkSyncTx {
            tx: ZkSyncTx::Transfer(Box::new(belated_transfer)),
            eth_sign_data: None,
        };
        let proposed_block = ProposedBlock {
            txs: vec![
                SignedTxVariant::Tx(premature_transfer.clone()),
                SignedTxVariant::Tx(belated_transfer.clone()),
                SignedTxVariant::Tx(correct_transfer.clone()),
            ],
            priority_ops: vec![],
        };

        tester
            .state_keeper
            .execute_proposed_block(proposed_block)
            .await;
        if let Some(CommitRequest::PendingBlock((block, _))) = tester.response_rx.next().await {
            assert_eq!(block.number, BlockNumber(1)); // It's the first block.

            assert_eq!(block.success_operations.len(), 1);
            assert_eq!(
                block.success_operations[0]
                    .get_executed_tx()
                    .unwrap()
                    .signed_tx
                    .hash(),
                correct_transfer.hash()
            );

            assert_eq!(block.failed_txs.len(), 2);
            assert_eq!(
                block.failed_txs[0].signed_tx.hash(),
                premature_transfer.hash()
            );
            assert_eq!(
                block.failed_txs[1].signed_tx.hash(),
                belated_transfer.hash()
            );
        } else {
            panic!("Block not stored");
        }
    }

    /// Checks that execution of failed transaction shouldn't change gas count.
    #[tokio::test]
    async fn gas_count_change() {
        let mut tester = StateKeeperTester::new(50, 5, 5);
        let initial_gas_count = tester
            .state_keeper
            .pending_block
            .gas_counter
            .commit_gas_limit();

        // Create withdraw which will fail.
        let withdraw = create_account_and_withdrawal(
            &mut tester,
            TokenId(0),
            AccountId(1),
            200u32,
            300u32,
            Default::default(),
        );
        let result = tester.state_keeper.apply_tx(&withdraw);

        assert!(result.is_ok());
        // Check that gas count shouldn't change
        assert_eq!(
            initial_gas_count,
            tester
                .state_keeper
                .pending_block
                .gas_counter
                .commit_gas_limit()
        );

        // Create two transfers which will fail.
        let first_transfer =
            create_account_and_transfer(&mut tester, TokenId(0), AccountId(2), 200u32, 300u32);
        let second_transfer =
            create_account_and_transfer(&mut tester, TokenId(0), AccountId(3), 200u32, 300u32);
        let proposed_block = ProposedBlock {
            txs: vec![SignedTxVariant::Batch(SignedTxsBatch {
                txs: vec![first_transfer, second_transfer],
                batch_id: 1,
                eth_signatures: Vec::new(),
            })],
            priority_ops: Vec::new(),
        };
        tester
            .state_keeper
            .execute_proposed_block(proposed_block)
            .await;
        // Check that gas count shouldn't change
        assert_eq!(
            initial_gas_count,
            tester
                .state_keeper
                .pending_block
                .gas_counter
                .commit_gas_limit()
        );

        // Create correct transfer.
        let third_transfer =
            create_account_and_transfer(&mut tester, TokenId(0), AccountId(4), 200u32, 100u32);

        let result = tester.state_keeper.apply_tx(&third_transfer);

        assert!(result.is_ok());
        // Check that gas count should increase
        assert!(
            initial_gas_count
                < tester
                    .state_keeper
                    .pending_block
                    .gas_counter
                    .commit_gas_limit()
        );
    }

    /// Calculates count of withdrawals that fit into block gas limit.
    fn withdrawals_fit_into_block() -> u32 {
        let mut tester = StateKeeperTester::new(1000, 1000, 1000);

        let withdraw = create_account_and_withdrawal(
            &mut tester,
            TokenId(0),
            AccountId(1),
            20u32,
            10u32,
            Default::default(),
        );
        let op = tester
            .state_keeper
            .state
            .zksync_tx_to_zksync_op(withdraw.tx)
            .unwrap();
        let mut count = 0;
        let mut gas_counter = GasCounter::new();
        while gas_counter.add_op(&op).is_ok() {
            count += 1;
        }
        count
    }

    /// Checks that block seals after reaching gas limit.
    #[tokio::test]
    async fn gas_limit_sealing() {
        let mut tester = StateKeeperTester::new(1000, 1000, 1000);

        let withdrawals_count = withdrawals_fit_into_block();

        // Create (withdrawals_count + 1) withdrawal
        let txs: Vec<_> = (0..=withdrawals_count)
            .map(|i| {
                let withdraw = create_account_and_withdrawal(
                    &mut tester,
                    TokenId(0),
                    AccountId(i + 1),
                    20u32,
                    10u32,
                    Default::default(),
                );
                SignedTxVariant::Tx(withdraw)
            })
            .collect();
        let last_withdraw = match txs.last().unwrap() {
            SignedTxVariant::Tx(tx) => tx.clone(),
            _ => panic!("Tx was expected"),
        };

        let proposed_block = ProposedBlock {
            txs,
            priority_ops: Vec::new(),
        };
        tester
            .state_keeper
            .execute_proposed_block(proposed_block)
            .await;

        // Checks that one block is sealed and pending block has only the last withdrawal.
        if !matches!(
            tester.response_rx.next().await,
            Some(CommitRequest::Block((_, _)))
        ) {
            panic!("Sealed block is not received");
        }
        if let Some(CommitRequest::PendingBlock((pending_block, _))) =
            tester.response_rx.next().await
        {
            assert_eq!(pending_block.success_operations.len(), 1);
            let tx = pending_block.success_operations.first().unwrap();
            match tx {
                ExecutedOperations::Tx(tx) => {
                    assert_eq!(tx.signed_tx.tx.hash(), last_withdraw.tx.hash());
                }
                _ => panic!("Tx was expected"),
            }
        } else {
            panic!("Pending block is not received");
        }
    }

    /// Checks that batch that doesn't fit into gas limit is processed correctly.
    #[tokio::test]
    async fn batch_gas_limit() {
        let mut tester = StateKeeperTester::new(1000, 1000, 1000);
        let withdrawals_count = withdrawals_fit_into_block();

        // Create (withdrawals_count + 1) withdrawal
        let txs: Vec<_> = (0..=withdrawals_count)
            .map(|i| {
                create_account_and_withdrawal(
                    &mut tester,
                    TokenId(0),
                    AccountId(i + 1),
                    20u32,
                    10u32,
                    Default::default(),
                )
            })
            .collect();

        let proposed_block = ProposedBlock {
            txs: vec![SignedTxVariant::Batch(SignedTxsBatch {
                txs,
                batch_id: 1,
                eth_signatures: Vec::new(),
            })],
            priority_ops: Vec::new(),
        };
        // Execute big batch.
        tester
            .state_keeper
            .execute_proposed_block(proposed_block)
            .await;
        if let Some(CommitRequest::PendingBlock((block, _))) = tester.response_rx.next().await {
            assert_eq!(block.failed_txs.len() as u32, withdrawals_count + 1);
            let expected_fail_reason =
                Some("Amount of gas required to process batch is too big".to_string());
            for tx in block.failed_txs {
                assert_eq!(tx.fail_reason, expected_fail_reason);
            }
        } else {
            panic!("Pending block is not received");
        }
    }
}
