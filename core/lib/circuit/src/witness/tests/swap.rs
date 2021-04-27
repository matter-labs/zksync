// External deps
use num::BigUint;
use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;
// Workspace deps
use zksync_state::{
    handler::TxHandler,
    state::{CollectedFee, ZkSyncState},
};
use zksync_types::{
    operations::SwapOp,
    tx::{Order, Swap, TimeRange},
    AccountId, TokenId,
};
// Local deps
use crate::witness::{
    swap::SwapWitness,
    tests::test_utils::{
        corrupted_input_test_scenario, generic_test_scenario, incorrect_op_test_scenario,
        WitnessTestAccount, BLOCK_TIMESTAMP,
    },
    utils::SigDataInput,
};
use zksync_crypto::params::number_of_processable_tokens;

struct TestSwap {
    accounts: (u32, u32),
    recipients: (u32, u32),
    submitter: u32,
    tokens: (u16, u16),
    amounts: (u64, u64),
    balances: (u64, u64, u64),
    first_price: (u64, u64),
    second_price: (u64, u64),
    fee_token: u16,
    fee: u64,
    test_accounts: Vec<WitnessTestAccount>,
}

type SwapSigDataInput = (SigDataInput, SigDataInput, SigDataInput);

impl TestSwap {
    fn create_accounts(&mut self) {
        if !self.test_accounts.is_empty() {
            return;
        }
        self.test_accounts = vec![
            WitnessTestAccount::new_with_token(
                AccountId(self.accounts.0),
                self.balances.0,
                TokenId(self.tokens.0),
            ),
            WitnessTestAccount::new_empty(AccountId(self.recipients.0)),
            WitnessTestAccount::new_with_token(
                AccountId(self.accounts.1),
                self.balances.1,
                TokenId(self.tokens.1),
            ),
            WitnessTestAccount::new_empty(AccountId(self.recipients.1)),
            WitnessTestAccount::new_with_token(
                AccountId(self.submitter),
                self.balances.2,
                TokenId(self.fee_token),
            ),
        ];
    }

    fn get_accounts(&self) -> &[WitnessTestAccount] {
        &self.test_accounts
    }

    fn get_op(&self) -> (SwapOp, SwapSigDataInput) {
        assert!(!self.test_accounts.is_empty());

        let order_0 = self.test_accounts[0].zksync_account.sign_order(
            TokenId(self.tokens.0),
            TokenId(self.tokens.1),
            BigUint::from(self.first_price.0),
            BigUint::from(self.first_price.1),
            BigUint::from(self.amounts.0),
            self.test_accounts[1].id,
            None,
            true,
            Default::default(),
        );

        let order_1 = self.test_accounts[2].zksync_account.sign_order(
            TokenId(self.tokens.1),
            TokenId(self.tokens.0),
            BigUint::from(self.second_price.0),
            BigUint::from(self.second_price.1),
            BigUint::from(self.amounts.1),
            self.test_accounts[3].id,
            None,
            true,
            Default::default(),
        );

        let swap_op = SwapOp {
            tx: self.test_accounts[4]
                .zksync_account
                .sign_swap(
                    (order_0.clone(), order_1.clone()),
                    (BigUint::from(self.amounts.0), BigUint::from(self.amounts.1)),
                    None,
                    true,
                    TokenId(self.fee_token),
                    "",
                    BigUint::from(self.fee),
                )
                .0,
            accounts: (self.test_accounts[0].id, self.test_accounts[2].id),
            recipients: (self.test_accounts[1].id, self.test_accounts[3].id),
            submitter: self.test_accounts[4].id,
        };

        // Additional data required for performing the operation.
        let input = (
            SigDataInput::from_order(&order_0).expect("SigDataInput creation failed"),
            SigDataInput::from_order(&order_1).expect("SigDataInput creation failed"),
            SigDataInput::from_swap_op(&swap_op).expect("SigDataInput creation failed"),
        );

        (swap_op, input)
    }
}

#[test]
#[ignore]
fn test_swap_success() {
    let mut test_swaps = vec![
        // Basic swap
        TestSwap {
            accounts: (1, 3),
            recipients: (2, 4),
            submitter: 5,
            tokens: (18, 19),
            fee_token: 0,
            amounts: (50, 100),
            fee: 25,
            balances: (100, 200, 50),
            first_price: (1, 2),
            second_price: (2, 1),
            test_accounts: vec![],
        },
        // Zero swap
        TestSwap {
            accounts: (1, 3),
            recipients: (2, 4),
            submitter: 5,
            tokens: (18, 19),
            fee_token: 0,
            amounts: (0, 0),
            fee: 0,
            balances: (100, 200, 50),
            first_price: (1, 2),
            second_price: (2, 1),
            test_accounts: vec![],
        },
        // Not exactly equal prices
        TestSwap {
            accounts: (1, 3),
            recipients: (2, 4),
            submitter: 5,
            tokens: (18, 19),
            fee_token: 0,
            amounts: (100, 100),
            fee: 25,
            balances: (100, 200, 50),
            first_price: (100, 99),
            second_price: (100, 99),
            test_accounts: vec![],
        },
    ];

    for test_swap in test_swaps.iter_mut() {
        test_swap.create_accounts();
        let (swap_op, input) = test_swap.get_op();

        generic_test_scenario::<SwapWitness<Bn256>, _>(
            test_swap.get_accounts(),
            swap_op,
            input,
            |state, op| {
                let fee = <ZkSyncState as TxHandler<Swap>>::apply_op(state, &op)
                    .expect("Operation failed")
                    .0
                    .unwrap();
                vec![fee]
            },
        );
    }
}
