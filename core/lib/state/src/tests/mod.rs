mod collect_fee;
mod operations;

use crate::state::ZkSyncState;
use num::BigUint;
use web3::types::H256;
use zksync_crypto::{
    priv_key_from_fs,
    rand::{Rng, SeedableRng, XorShiftRng},
    PrivateKey,
};
use zksync_types::{
    tx::PackedEthSignature, Account, AccountId, AccountUpdate, PubKeyHash, SignedZkSyncTx, TokenId,
    ZkSyncPriorityOp, ZkSyncTx, NFT,
};

type BoundAccountUpdates = [(AccountId, AccountUpdate)];

pub enum AccountState {
    Locked,
    Unlocked,
}

pub struct PlasmaTestBuilder {
    rng: XorShiftRng,
    state: ZkSyncState,
}

impl Default for PlasmaTestBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PlasmaTestBuilder {
    pub fn new() -> Self {
        Self {
            rng: XorShiftRng::from_seed([1, 2, 3, 4]),
            state: ZkSyncState::empty(),
        }
    }

    pub fn mint_nft(
        &mut self,
        token_id: TokenId,
        content_hash: H256,
        recipient_id: AccountId,
        creator_id: AccountId,
    ) {
        let creator_address = self.state.get_account(creator_id).unwrap().address;
        let nft = NFT::new(
            token_id,
            0,
            creator_id,
            creator_address,
            Default::default(),
            None,
            content_hash,
        );
        self.state.nfts.insert(token_id, nft);
        self.set_balance(recipient_id, token_id, 1u32);
    }

    pub fn add_account(&mut self, state: AccountState) -> (AccountId, Account, PrivateKey) {
        let account_id = self.state.get_free_account_id();

        let sk = priv_key_from_fs(self.rng.gen());

        let eth_sk = H256::random();
        let address = PackedEthSignature::address_from_private_key(&eth_sk)
            .expect("Can't get address from the ETH secret key");

        let mut account = Account::default_with_address(&address);
        if let AccountState::Unlocked = state {
            account.pub_key_hash = PubKeyHash::from_privkey(&sk);
        }

        self.state.insert_account(account_id, account.clone());

        (account_id, account, sk)
    }

    pub fn set_balance<B: Into<BigUint>>(
        &mut self,
        account_id: AccountId,
        token_id: TokenId,
        amount: B,
    ) {
        let mut account = self
            .state
            .get_account(account_id)
            .expect("account doesn't exist");

        account.set_balance(token_id, amount.into());

        self.state.insert_account(account_id, account);
    }

    pub fn test_tx_success(&mut self, tx: ZkSyncTx, expected_updates: &BoundAccountUpdates) {
        let mut state_clone = self.state.clone();
        let op_success = self.state.execute_tx(tx).expect("transaction failed");
        self.compare_updates(
            expected_updates,
            op_success.updates.as_slice(),
            &mut state_clone,
        );
    }

    pub fn test_tx_fail(&mut self, tx: ZkSyncTx, expected_error_message: &str) {
        let error = self
            .state
            .execute_tx(tx)
            .expect_err("transaction didn't fail");

        assert_eq!(
            error.to_string().as_str(),
            expected_error_message,
            "unexpected error message"
        );
    }

    pub fn test_txs_batch_success(
        &mut self,
        txs: &[SignedZkSyncTx],
        expected_updates: &BoundAccountUpdates,
    ) {
        let mut state_clone = self.state.clone();
        let op_successes = self.state.execute_txs_batch(txs);
        let mut updates: Vec<(AccountId, AccountUpdate)> = Vec::new();
        for result in op_successes {
            updates.append(&mut result.unwrap().updates);
        }
        self.compare_updates(expected_updates, &updates, &mut state_clone);
    }

    pub fn test_txs_batch_fail(&mut self, txs: &[SignedZkSyncTx], expected_error_message: &str) {
        let state_clone = self.state.clone();
        let op_errors = self.state.execute_txs_batch(txs);
        for error in op_errors {
            assert_eq!(
                error.unwrap_err().to_string().as_str(),
                expected_error_message,
                "unexpected error message"
            );
        }
        assert_eq!(
            self.state.root_hash(),
            state_clone.root_hash(),
            "state has changed, but it should not"
        );
    }

    pub fn test_priority_op_success(
        &mut self,
        op: ZkSyncPriorityOp,
        expected_updates: &BoundAccountUpdates,
    ) {
        let mut state_clone = self.state.clone();
        let op_success = self.state.execute_priority_op(op);
        self.compare_updates(
            expected_updates,
            op_success.updates.as_slice(),
            &mut state_clone,
        );
    }

    pub fn compare_updates(
        &self,
        expected_updates: &BoundAccountUpdates,
        actual_updates: &BoundAccountUpdates,
        state_clone: &mut ZkSyncState,
    ) {
        assert_eq!(expected_updates, actual_updates, "unexpected updates");

        state_clone.apply_updates(expected_updates);

        assert_eq!(
            self.state.root_hash(),
            state_clone.root_hash(),
            "returned updates don't match real state changes"
        );
    }
}

#[test]
fn test_tree_state() {
    let mut state = ZkSyncState::empty();
    let empty_root = state.root_hash();
    state.insert_account(AccountId(0), Default::default());
    let empty_acc_root = state.root_hash();

    // Tree contains "empty" accounts by default, inserting an empty account shouldn't change state.
    assert_eq!(empty_root, empty_acc_root);

    let mut balance_account = Account::default();
    balance_account.set_balance(TokenId(0), 100u64.into());
    state.insert_account(AccountId(1), balance_account.clone());
    let balance_root = state.root_hash();

    balance_account.set_balance(TokenId(0), 0u64.into());
    state.insert_account(AccountId(1), balance_account.clone());
    let no_balance_root = state.root_hash();

    // Account with manually set 0 token amount should be considered empty as well.
    assert_eq!(no_balance_root, empty_acc_root);

    balance_account.set_balance(TokenId(0), 100u64.into());
    state.insert_account(AccountId(1), balance_account);
    let restored_balance_root = state.root_hash();

    // After we restored previously observed balance, root should be identical.
    assert_eq!(balance_root, restored_balance_root);
}
