// External deps
use num::{BigUint, Zero};
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
    is_limit_order: (bool, bool),
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
            WitnessTestAccount::new_with_token(
                AccountId(self.accounts.1),
                self.balances.1,
                TokenId(self.tokens.1),
            ),
            WitnessTestAccount::new_with_token(
                AccountId(self.submitter),
                self.balances.2,
                TokenId(self.fee_token),
            ),
        ];
    }

    fn get_accounts(&self) -> Vec<WitnessTestAccount> {
        let ids = [self.accounts.0, self.accounts.1, self.submitter];
        let mut accounts = self.test_accounts.clone();
        if !ids.contains(&self.recipients.0) {
            accounts.push(WitnessTestAccount::new_empty(AccountId(self.recipients.0)));
        }
        if !ids.contains(&self.recipients.1) {
            accounts.push(WitnessTestAccount::new_empty(AccountId(self.recipients.1)));
        }
        accounts
    }

    fn get_op(&self) -> (SwapOp, SwapSigDataInput, (Order, Order)) {
        assert!(!self.test_accounts.is_empty());

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

        let order_0 = self.test_accounts[0].zksync_account.sign_order(
            TokenId(self.tokens.0),
            TokenId(self.tokens.1),
            BigUint::from(self.first_price.0),
            BigUint::from(self.first_price.1),
            amount_0,
            AccountId(self.recipients.0),
            None,
            self.is_limit_order.0,
            Default::default(),
        );

        let order_1 = self.test_accounts[1].zksync_account.sign_order(
            TokenId(self.tokens.1),
            TokenId(self.tokens.0),
            BigUint::from(self.second_price.0),
            BigUint::from(self.second_price.1),
            amount_1,
            AccountId(self.recipients.1),
            None,
            self.is_limit_order.1,
            Default::default(),
        );

        let swap_op = SwapOp {
            tx: self.test_accounts[2]
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
            accounts: (self.test_accounts[0].id, self.test_accounts[1].id),
            recipients: (AccountId(self.recipients.0), AccountId(self.recipients.1)),
            submitter: self.test_accounts[2].id,
        };

        // Additional data required for performing the operation.
        let input = (
            SigDataInput::from_order(&order_0).expect("SigDataInput creation failed"),
            SigDataInput::from_order(&order_1).expect("SigDataInput creation failed"),
            SigDataInput::from_swap_op(&swap_op).expect("SigDataInput creation failed"),
        );

        (swap_op, input, (order_0, order_1))
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
            is_limit_order: (false, false),
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
            is_limit_order: (false, false),
            test_accounts: vec![],
        },
        // Not exactly equal, but compatible prices
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
            is_limit_order: (false, false),
            test_accounts: vec![],
        },
        // Default recipients
        TestSwap {
            accounts: (1, 3),
            recipients: (1, 3),
            submitter: 5,
            tokens: (18, 19),
            fee_token: 0,
            amounts: (50, 100),
            fee: 25,
            balances: (100, 200, 50),
            first_price: (1, 2),
            second_price: (2, 1),
            is_limit_order: (false, false),
            test_accounts: vec![],
        },
        // Weird case for recipients
        TestSwap {
            accounts: (1, 3),
            recipients: (3, 1),
            submitter: 5,
            tokens: (18, 19),
            fee_token: 0,
            amounts: (50, 100),
            fee: 25,
            balances: (100, 200, 50),
            first_price: (1, 2),
            second_price: (2, 1),
            is_limit_order: (false, false),
            test_accounts: vec![],
        },
        // Submitter is one of the recipients
        TestSwap {
            accounts: (1, 3),
            recipients: (2, 5),
            submitter: 5,
            tokens: (18, 19),
            fee_token: 0,
            amounts: (50, 100),
            fee: 25,
            balances: (100, 200, 50),
            first_price: (1, 2),
            second_price: (2, 1),
            is_limit_order: (false, false),
            test_accounts: vec![],
        },
        // Basic limit order
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
            is_limit_order: (true, true),
            test_accounts: vec![],
        },
    ];

    for test_swap in test_swaps.iter_mut() {
        test_swap.create_accounts();
        let (swap_op, input, _) = test_swap.get_op();

        generic_test_scenario::<SwapWitness<Bn256>, _>(
            &test_swap.get_accounts(),
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
#[test]
#[ignore]
fn test_swap_limit_orders() {
    let mut test_swap = TestSwap {
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
        is_limit_order: (true, true),
        test_accounts: vec![],
    };

    test_swap.create_accounts();
    let (swap_op, input, orders) = test_swap.get_op();
    let mut test_accounts = test_swap.get_accounts();

    generic_test_scenario::<SwapWitness<Bn256>, _>(
        &test_accounts,
        swap_op,
        input.clone(),
        |state, op| {
            let fee = <ZkSyncState as TxHandler<Swap>>::apply_op(state, &op)
                .expect("Operation failed")
                .0
                .unwrap();
            vec![fee]
        },
    );

    let new_submitter = WitnessTestAccount::new_with_token(AccountId(6), 24, TokenId(10));

    // Using same signed limit orders but different submitter
    let second_swap_op = SwapOp {
        tx: new_submitter
            .zksync_account
            .sign_swap(
                (orders.0, orders.1),
                (BigUint::from(40u8), BigUint::from(80u8)),
                None,
                true,
                TokenId(10),
                "",
                BigUint::from(20u8),
            )
            .0,
        accounts: (AccountId(1), AccountId(3)),
        recipients: (AccountId(2), AccountId(4)),
        submitter: AccountId(6),
    };

    test_accounts.push(new_submitter);
    let second_swap_input =
        SigDataInput::from_swap_op(&second_swap_op).expect("SigDataInput creation failed");

    generic_test_scenario::<SwapWitness<Bn256>, _>(
        &test_accounts,
        second_swap_op,
        (input.0, input.1, second_swap_input),
        |state, op| {
            let fee = <ZkSyncState as TxHandler<Swap>>::apply_op(state, &op)
                .expect("Operation failed")
                .0
                .unwrap();
            vec![fee]
        },
    );
}
