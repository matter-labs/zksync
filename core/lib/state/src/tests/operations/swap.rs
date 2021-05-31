use crate::tests::{AccountState::*, PlasmaTestBuilder};
use num::{BigUint, Zero};
use web3::types::H160;
use zksync_crypto::PrivateKey;
use zksync_types::{
    Account, AccountId, AccountUpdate, Nonce, Order, SignedZkSyncTx, Swap, TokenId, ZkSyncTx,
};

type TestAccount = (AccountId, Account, PrivateKey);

struct TestSwap {
    accounts: (usize, usize),
    recipients: (usize, usize),
    submitter: usize,
    tokens: (u32, u32),
    amounts: (u64, u64),
    balances: (u64, u64, u64),
    first_price: (u64, u64),
    second_price: (u64, u64),
    fee_token: u32,
    fee: u64,
    is_limit_order: (bool, bool),
    test_accounts: Vec<TestAccount>,
}

impl TestSwap {
    fn test_success(&self, mut tb: PlasmaTestBuilder) {
        let (account_0_id, account_0, account_0_sk) = &self.test_accounts[self.accounts.0];
        let (account_1_id, account_1, account_1_sk) = &self.test_accounts[self.accounts.1];
        let (recipient_0_id, recipient_0, _) = &self.test_accounts[self.recipients.0];
        let (recipient_1_id, recipient_1, _) = &self.test_accounts[self.recipients.1];
        let (submitter_id, submitter, submitter_sk) = &self.test_accounts[self.submitter];

        let token_0 = TokenId(self.tokens.0);
        let token_1 = TokenId(self.tokens.1);
        let fee_token = TokenId(self.fee_token);
        let fee = BigUint::from(self.fee);

        let amount_0 = if self.is_limit_order.0 {
            BigUint::zero()
        } else {
            BigUint::from(self.amounts.0)
        };

        let amount_1 = if self.is_limit_order.1 {
            BigUint::zero()
        } else {
            BigUint::from(self.amounts.1)
        };

        let balances = (
            BigUint::from(self.balances.0),
            BigUint::from(self.balances.1),
            BigUint::from(self.balances.2),
        );

        tb.set_balance(*account_0_id, token_0, balances.0.clone());
        tb.set_balance(*account_1_id, token_1, balances.1.clone());
        tb.set_balance(*submitter_id, fee_token, balances.2.clone());

        let order_0 = Order::new_signed(
            *account_0_id,
            recipient_0.address,
            account_0.nonce,
            token_0,
            token_1,
            (
                BigUint::from(self.first_price.0),
                BigUint::from(self.first_price.1),
            ),
            amount_0.clone(),
            Default::default(),
            &&account_0_sk,
        )
        .expect("order creation failed");

        let order_1 = Order::new_signed(
            *account_1_id,
            recipient_1.address,
            account_1.nonce,
            token_1,
            token_0,
            (
                BigUint::from(self.second_price.0),
                BigUint::from(self.second_price.1),
            ),
            amount_1.clone(),
            Default::default(),
            &account_1_sk,
        )
        .expect("order creation failed");

        let swap = Swap::new_signed(
            *submitter_id,
            submitter.address,
            submitter.nonce,
            (order_0, order_1),
            (BigUint::from(self.amounts.0), BigUint::from(self.amounts.1)),
            fee.clone(),
            fee_token,
            &submitter_sk,
        )
        .expect("swap creation failed");

        tb.test_tx_success(
            swap.into(),
            &[
                (
                    *account_0_id,
                    AccountUpdate::UpdateBalance {
                        old_nonce: account_0.nonce,
                        new_nonce: account_0.nonce + 1,
                        balance_update: (token_0, balances.0.clone(), balances.0 - &amount_0),
                    },
                ),
                (
                    *recipient_1_id,
                    AccountUpdate::UpdateBalance {
                        old_nonce: recipient_1.nonce,
                        new_nonce: recipient_1.nonce,
                        balance_update: (token_0, BigUint::zero(), amount_0),
                    },
                ),
                (
                    *account_1_id,
                    AccountUpdate::UpdateBalance {
                        old_nonce: account_1.nonce,
                        new_nonce: account_1.nonce + 1,
                        balance_update: (token_1, balances.1.clone(), balances.1 - &amount_1),
                    },
                ),
                (
                    *recipient_0_id,
                    AccountUpdate::UpdateBalance {
                        old_nonce: recipient_0.nonce,
                        new_nonce: recipient_0.nonce,
                        balance_update: (token_1, BigUint::zero(), amount_1),
                    },
                ),
                (
                    *submitter_id,
                    AccountUpdate::UpdateBalance {
                        old_nonce: submitter.nonce,
                        new_nonce: submitter.nonce + 1,
                        balance_update: (fee_token, balances.2.clone(), balances.2 - &fee),
                    },
                ),
            ],
        )
    }
}

#[test]
fn swap_success() {
    let mut tb = PlasmaTestBuilder::new();

    let test_swap = TestSwap {
        accounts: (0, 1),
        recipients: (2, 3),
        submitter: 4,
        tokens: (18, 19),
        fee_token: 0,
        amounts: (50, 100),
        fee: 25,
        balances: (100, 200, 50),
        first_price: (1, 2),
        second_price: (2, 1),
        is_limit_order: (false, false),
        test_accounts: vec![
            tb.add_account(Unlocked),
            tb.add_account(Unlocked),
            tb.add_account(Unlocked),
            tb.add_account(Unlocked),
            tb.add_account(Unlocked),
        ],
    };

    test_swap.test_success(tb);
}
