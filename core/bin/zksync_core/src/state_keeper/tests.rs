use super::{CommitRequest, ZkSyncStateInitParams, ZkSyncStateKeeper};
use crate::mempool::ProposedBlock;
use futures::{channel::mpsc, stream::StreamExt};
use num::BigUint;
use zksync_crypto::{
    priv_key_from_fs,
    rand::{Rng, SeedableRng, XorShiftRng},
    PrivateKey,
};
use zksync_types::{mempool::SignedTxVariant, tx::PackedEthSignature, AccountId, H160, *};

struct StateKeeperTester {
    state_keeper: ZkSyncStateKeeper,
    response_rx: mpsc::Receiver<CommitRequest>,
    fee_collector: AccountId,
}

impl StateKeeperTester {
    fn new(
        available_chunk_size: usize,
        max_iterations: usize,
        fast_iterations: usize,
        number_of_withdrawals: usize,
    ) -> Self {
        const CHANNEL_SIZE: usize = 32768;
        let (_request_tx, request_rx) = mpsc::channel(CHANNEL_SIZE);
        let (response_tx, response_rx) = mpsc::channel(CHANNEL_SIZE);

        let mut fee_collector = Account::default();
        fee_collector.address = H160::random();

        let mut init_params = ZkSyncStateInitParams::default();
        init_params.insert_account(0, fee_collector.clone());

        let state_keeper = ZkSyncStateKeeper::new(
            init_params,
            fee_collector.address,
            request_rx,
            response_tx,
            vec![available_chunk_size],
            max_iterations,
            fast_iterations,
            number_of_withdrawals,
        );

        Self {
            state_keeper,
            response_rx,
            fee_collector: 0,
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

        let mut account = Account::default();
        account.address = address;
        account.pub_key_hash = PubKeyHash::from_privkey(&sk);
        self.state_keeper
            .state
            .insert_account(account_id, account.clone());
        (account, sk)
    }
}

fn create_account_and_withdrawal<B: Into<BigUint>>(
    tester: &mut StateKeeperTester,
    token_id: TokenId,
    account_id: AccountId,
    balance: B,
    withdraw_amount: B,
) -> SignedZkSyncTx {
    create_account_and_withdrawal_impl(
        tester,
        token_id,
        account_id,
        balance,
        withdraw_amount,
        false,
    )
}

fn create_account_and_fast_withdrawal<B: Into<BigUint>>(
    tester: &mut StateKeeperTester,
    token_id: TokenId,
    account_id: AccountId,
    balance: B,
    withdraw_amount: B,
) -> SignedZkSyncTx {
    create_account_and_withdrawal_impl(tester, token_id, account_id, balance, withdraw_amount, true)
}

fn create_account_and_withdrawal_impl<B: Into<BigUint>>(
    tester: &mut StateKeeperTester,
    token_id: TokenId,
    account_id: AccountId,
    balance: B,
    withdraw_amount: B,
    fast: bool,
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
        eth_hash: vec![],
        eth_block: 0,
    }
}

mod apply_priority_op {
    use super::*;

    /// Checks if deposit is processed correctly by the state_keeper
    #[test]
    fn success() {
        let mut tester = StateKeeperTester::new(6, 1, 1, 0);
        let old_pending_block = tester.state_keeper.pending_block.clone();
        let deposit = create_deposit(0, 145u32);
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
        let mut tester = StateKeeperTester::new(1, 1, 1, 0);
        let deposit = create_deposit(0, 1u32);
        let result = tester.state_keeper.apply_priority_op(deposit);
        assert!(result.is_err());
    }
}

mod apply_tx {
    use super::*;

    /// Checks if withdrawal is processed correctly by the state_keeper
    #[test]
    fn success() {
        let mut tester = StateKeeperTester::new(6, 1, 1, 1);
        let old_pending_block = tester.state_keeper.pending_block.clone();
        let withdraw = create_account_and_withdrawal(&mut tester, 0, 1, 200u32, 145u32);
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
        assert_eq!(pending_block.withdrawals_amount, 1);
    }

    /// Checks if fast withdrawal makes fast processing required
    #[test]
    fn fast_withdrawal() {
        let mut tester = StateKeeperTester::new(6, 1, 1, 1);
        let old_pending_block = tester.state_keeper.pending_block.clone();
        let withdraw = create_account_and_fast_withdrawal(&mut tester, 0, 1, 200u32, 145u32);
        let result = tester.state_keeper.apply_tx(&withdraw);
        let pending_block = tester.state_keeper.pending_block;

        assert!(result.is_ok());
        assert_eq!(old_pending_block.fast_processing_required, false);
        assert_eq!(pending_block.fast_processing_required, true);
    }

    /// Checks if withdrawal that will fail is processed correctly
    #[test]
    fn failure() {
        let mut tester = StateKeeperTester::new(6, 1, 1, 1);
        let old_pending_block = tester.state_keeper.pending_block.clone();
        let withdraw = create_account_and_withdrawal(&mut tester, 0, 1, 100u32, 145u32);
        let result = tester.state_keeper.apply_tx(&withdraw);
        let pending_block = tester.state_keeper.pending_block;

        assert!(result.is_ok());
        assert!(pending_block.chunks_left == old_pending_block.chunks_left);
        assert_eq!(
            pending_block.pending_op_block_index,
            old_pending_block.pending_op_block_index
        );
        assert!(pending_block.account_updates.is_empty());
        assert!(!pending_block.failed_txs.is_empty());
        assert!(pending_block.collected_fees.is_empty());
        assert_eq!(pending_block.withdrawals_amount, 1);
    }

    /// Checks if processing withdrawal fails because of
    /// small number of chunks left in the block
    #[test]
    fn not_enough_chunks() {
        let mut tester = StateKeeperTester::new(1, 1, 1, 1);
        let withdraw = create_account_and_withdrawal(&mut tester, 0, 1, 200u32, 145u32);
        let result = tester.state_keeper.apply_tx(&withdraw);
        assert!(result.is_err());
    }

    /// Checks if processing withdrawal fails because of
    /// small number of withdrawals_per_block
    #[test]
    fn withdrawals_limit_reached() {
        let mut tester = StateKeeperTester::new(6, 1, 1, 0);
        let withdraw = create_account_and_withdrawal(&mut tester, 0, 1, 200u32, 145u32);
        let result = tester.state_keeper.apply_tx(&withdraw);
        assert!(result.is_err());
    }

    /// Checks if processing withdrawal fails because the gas limit is reached.
    /// This sends 46 withdrawals (very ineficcient, but all constants in
    /// GasCounter are hardcoded, so I see no way out)
    #[test]
    fn gas_limit_reached() {
        let withdrawals_number = 46;
        let mut tester = StateKeeperTester::new(6 * withdrawals_number, 1, 1, withdrawals_number);
        for i in 1..=withdrawals_number {
            let withdrawal =
                create_account_and_withdrawal(&mut tester, 0, i as u32, 200u32, 145u32);
            let result = tester.state_keeper.apply_tx(&withdrawal);
            if i < withdrawals_number {
                assert!(result.is_ok())
            } else {
                assert!(result.is_err())
            }
        }
    }
}

/// Checks if block sealing is done correctly by sealing a block
/// with 1 priority_op, 1 succeeded tx, 1 failed tx
#[tokio::test]
async fn seal_pending_block() {
    let mut tester = StateKeeperTester::new(20, 3, 3, 2);
    let good_withdraw = create_account_and_withdrawal(&mut tester, 0, 1, 200u32, 145u32);
    let bad_withdraw = create_account_and_withdrawal(&mut tester, 2, 2, 100u32, 145u32);
    let deposit = create_deposit(0, 12u32);

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
            .get_balance(0);
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
    let mut tester = StateKeeperTester::new(20, 3, 3, 2);
    let good_withdraw = create_account_and_withdrawal(&mut tester, 0, 1, 200u32, 145u32);
    let bad_withdraw = create_account_and_withdrawal(&mut tester, 2, 2, 100u32, 145u32);
    let deposit = create_deposit(0, 12u32);

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

    /// Checks if executing an empty proposed_block is done correctly
    #[tokio::test]
    async fn empty() {
        let mut tester = StateKeeperTester::new(1, 1, 1, 1);
        let proposed_block = ProposedBlock {
            txs: vec![],
            priority_ops: vec![],
        };
        let pending_block_iteration = tester.state_keeper.pending_block.pending_block_iteration;
        tester
            .state_keeper
            .execute_proposed_block(proposed_block)
            .await;
        if let Some(CommitRequest::PendingBlock(_)) = tester.response_rx.next().await {
            assert_eq!(
                pending_block_iteration,
                tester.state_keeper.pending_block.pending_block_iteration
            );
        } else {
            panic!("Empty block not stored");
        }
    }

    /// Checks if executing a small proposed_block is done correctly
    #[tokio::test]
    async fn small() {
        let mut tester = StateKeeperTester::new(20, 3, 3, 2);
        let good_withdraw = create_account_and_withdrawal(&mut tester, 0, 1, 200u32, 145u32);
        let bad_withdraw = create_account_and_withdrawal(&mut tester, 2, 2, 100u32, 145u32);
        let deposit = create_deposit(0, 12u32);
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
        let mut tester = StateKeeperTester::new(12, 3, 3, 2);
        let good_withdraw = create_account_and_withdrawal(&mut tester, 0, 1, 200u32, 145u32);
        let bad_withdraw = create_account_and_withdrawal(&mut tester, 2, 2, 100u32, 145u32);
        let deposit = create_deposit(0, 12u32);
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
    /// There are more withdrawals than one can fit in 1 block,
    /// so 1 block should get sealed in the process
    #[tokio::test]
    async fn few_withdrawals() {
        let mut tester = StateKeeperTester::new(20, 3, 3, 1);
        let good_withdraw = create_account_and_withdrawal(&mut tester, 0, 1, 200u32, 145u32);
        let bad_withdraw = create_account_and_withdrawal(&mut tester, 2, 2, 100u32, 145u32);
        let deposit = create_deposit(0, 12u32);
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
        let mut tester = StateKeeperTester::new(20, 0, 0, 2);
        let good_withdraw = create_account_and_withdrawal(&mut tester, 0, 1, 200u32, 145u32);
        let bad_withdraw = create_account_and_withdrawal(&mut tester, 2, 2, 100u32, 145u32);
        let deposit = create_deposit(0, 12u32);
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

        let mut tester = StateKeeperTester::new(6, MAX_ITERATIONS, FAST_ITERATIONS, 2);
        let withdraw = create_account_and_fast_withdrawal(&mut tester, 0, 1, 200u32, 145u32);

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
}
