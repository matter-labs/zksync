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

#[test]
#[ignore]
fn test_swap_success() {
    let account_0 = WitnessTestAccount::new_with_token(AccountId(1), 100, TokenId(2));
    let recipient_0 = WitnessTestAccount::new_empty(AccountId(2));
    let account_1 = WitnessTestAccount::new_with_token(AccountId(3), 200, TokenId(4));
    let recipient_1 = WitnessTestAccount::new_empty(AccountId(4));
    let submitter = WitnessTestAccount::new_with_token(AccountId(5), 50, TokenId(0));

    let amount_0 = BigUint::from(50u8);
    let amount_1 = BigUint::from(100u8);

    let order_0 = account_0.zksync_account.sign_order(
        TokenId(2),
        TokenId(4),
        BigUint::from(1u8),
        BigUint::from(2u8),
        amount_0.clone(),
        recipient_0.id,
        None,
        true,
        Default::default(),
    );

    let order_1 = account_1.zksync_account.sign_order(
        TokenId(4),
        TokenId(2),
        BigUint::from(2u8),
        BigUint::from(1u8),
        amount_1.clone(),
        recipient_1.id,
        None,
        true,
        Default::default(),
    );

    let swap_op = SwapOp {
        tx: submitter
            .zksync_account
            .sign_swap(
                (order_0.clone(), order_1.clone()),
                (amount_0, amount_1),
                None,
                true,
                TokenId(0),
                "ETH",
                BigUint::from(25u8),
            )
            .0,
        accounts: (account_0.id, account_1.id),
        recipients: (recipient_0.id, recipient_1.id),
        submitter: submitter.id,
    };

    // Additional data required for performing the operation.
    let input = (
        SigDataInput::from_order(&order_0).expect("SigDataInput creation failed"),
        SigDataInput::from_order(&order_1).expect("SigDataInput creation failed"),
        SigDataInput::from_swap_op(&swap_op).expect("SigDataInput creation failed"),
    );

    generic_test_scenario::<SwapWitness<Bn256>, _>(
        &[account_0, recipient_0, account_1, recipient_1, submitter],
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
