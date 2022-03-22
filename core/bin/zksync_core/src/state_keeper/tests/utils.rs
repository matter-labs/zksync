use crate::committer::{AppliedUpdatesRequest, BlockCommitRequest};
use crate::state_keeper::{CommitRequest, ZkSyncStateInitParams, ZkSyncStateKeeper};
use chrono::Utc;
use futures::{channel::mpsc, stream::StreamExt};
use num::BigUint;
use zksync_crypto::{
    priv_key_from_fs,
    rand::{Rng, SeedableRng, XorShiftRng},
    PrivateKey,
};
use zksync_mempool::ProposedBlock;
use zksync_types::block::{IncompleteBlock, PendingBlock};
use zksync_types::tx::TimeRange;
use zksync_types::{
    mempool::SignedTxVariant, mempool::SignedTxsBatch, tx::PackedEthSignature, AccountId, H160, *,
};

pub struct StateKeeperTester {
    pub state_keeper: ZkSyncStateKeeper,
    pub response_rx: mpsc::Receiver<CommitRequest>,
    pub fee_collector: AccountId,
}

impl StateKeeperTester {
    pub fn new(available_chunk_size: usize, max_iterations: usize, fast_iterations: usize) -> Self {
        const CHANNEL_SIZE: usize = 32768;
        let (events_sender, _events_receiver) = mpsc::channel(CHANNEL_SIZE);
        let (request_tx, _request_rx) = mpsc::channel(CHANNEL_SIZE);
        let (response_tx, response_rx) = mpsc::channel(CHANNEL_SIZE);

        let fee_collector = Account::default_with_address(&H160::random());

        let mut init_params = ZkSyncStateInitParams::default();
        init_params.insert_account(AccountId(0), fee_collector.clone());

        let (state_keeper, _root_hash_calculator) = ZkSyncStateKeeper::new(
            init_params,
            fee_collector.address,
            response_tx,
            request_tx,
            vec![available_chunk_size],
            max_iterations,
            fast_iterations,
            events_sender,
        );

        Self {
            state_keeper,
            response_rx,
            fee_collector: AccountId(0),
        }
    }

    pub fn set_balance(
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

    pub fn add_account(&mut self, account_id: AccountId) -> (Account, PrivateKey) {
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

    /// Ensures that `PendingBlock` is sent to the channel.
    pub async fn assert_pending(&mut self) {
        assert!(
            matches!(
                self.response_rx.next().await,
                Some(CommitRequest::PendingBlock(_))
            ),
            "Expected pending block to be sent"
        );
    }

    /// Ensures that `PendingBlock` is sent to the channel.
    /// Executes provided closure on the received pending block.
    pub async fn assert_pending_with(&mut self, f: impl FnOnce(PendingBlock)) {
        if let Some(CommitRequest::PendingBlock((block, _))) = self.response_rx.next().await {
            f(block);
        } else {
            panic!("Expected pending block to be sent");
        }
    }

    /// Similar to `assert_pending_with`, but returns the whole update instead of using a closure.
    /// Useful when you need to interact with `tester`.
    pub async fn unwrap_pending_update(&mut self) -> (PendingBlock, AppliedUpdatesRequest) {
        if let Some(CommitRequest::PendingBlock((block, updates))) = self.response_rx.next().await {
            (block, updates)
        } else {
            panic!("Expected pending block to be sent");
        }
    }

    /// Ensures that block is sealed by the state keeper.
    pub async fn assert_sealed(&mut self) {
        // Pending block is *always* sent, even if block was sealed.
        assert!(
            matches!(
                self.response_rx.next().await,
                Some(CommitRequest::PendingBlock(_))
            ),
            "Expected block sealing, didn't receive a pending block"
        );
        assert!(
            matches!(
                self.response_rx.next().await,
                Some(CommitRequest::SealIncompleteBlock(_))
            ),
            "Expected block sealing, didn't receive an incomplete block"
        );
    }

    /// Ensures that block is sealed by the state keeper.
    /// Executes provided closure on the received incomplete block.
    pub async fn assert_sealed_with(&mut self, f: impl FnOnce(IncompleteBlock)) {
        // Pending block is *always* sent, even if block was sealed.
        assert!(
            matches!(
                self.response_rx.next().await,
                Some(CommitRequest::PendingBlock(_))
            ),
            "Expected block sealing, didn't receive a pending block"
        );

        if let Some(CommitRequest::SealIncompleteBlock((block, _))) = self.response_rx.next().await
        {
            f(block.block);
        } else {
            panic!("Expected pending block to be sent");
        }
    }

    /// Similar to `assert_sealed_with`, but returns the whole update instead of using a closure.
    /// Useful when you need to interact with `tester`.
    pub async fn unwrap_sealed_update(&mut self) -> (BlockCommitRequest, AppliedUpdatesRequest) {
        // Pending block is *always* sent, even if block was sealed.
        assert!(
            matches!(
                self.response_rx.next().await,
                Some(CommitRequest::PendingBlock(_))
            ),
            "Expected block sealing, didn't receive a pending block"
        );

        if let Some(CommitRequest::SealIncompleteBlock((block, updates))) =
            self.response_rx.next().await
        {
            (block, updates)
        } else {
            panic!("Expected pending block to be sent");
        }
    }

    /// Ensures that there are no messages in the channel so far.
    pub async fn assert_empty(&mut self) {
        let next_block = self.response_rx.try_next();
        assert!(
            next_block.is_err(),
            "Something was in the channel while it was expected to be empty: {:?}",
            next_block
        );
    }
}

pub fn create_account_and_transfer<B: Into<BigUint>>(
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
        created_at: Utc::now(),
    }
}

pub fn create_account_and_withdrawal<B: Into<BigUint>>(
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

pub fn create_account_and_fast_withdrawal<B: Into<BigUint>>(
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

#[allow(clippy::too_many_arguments)]
pub fn create_account_and_withdrawal_impl<B: Into<BigUint>>(
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
        created_at: Utc::now(),
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
        eth_block_index: None,
    }
}

pub async fn apply_single_transfer(tester: &mut StateKeeperTester) {
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

pub async fn apply_batch_with_two_transfers(tester: &mut StateKeeperTester) {
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
