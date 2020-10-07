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
use zksync_types::tx::PackedEthSignature;
use zksync_types::{
    Account, AccountId, AccountUpdate, PubKeyHash, TokenId, ZkSyncPriorityOp, ZkSyncTx,
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

    pub fn add_account(&mut self, state: AccountState) -> (AccountId, Account, PrivateKey) {
        let account_id = self.state.get_free_account_id();

        let sk = priv_key_from_fs(self.rng.gen());

        let eth_sk = H256::random();
        let address = PackedEthSignature::address_from_private_key(&eth_sk)
            .expect("Can't get address from the ETH secret key");

        let mut account = Account::default();
        account.address = address;
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
