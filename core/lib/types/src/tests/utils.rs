use crate::eip712_signature::Eip712Domain;
use crate::tx::{
    ChangePubKey, ChangePubKeyEIP712Data, ChangePubKeyEthAuthData, PackedEthSignature,
};
use crate::*;
use chrono::Utc;
use zksync_crypto::priv_key_from_fs;
use zksync_crypto::rand::{Rng, XorShiftRng};

pub fn create_full_exit_op() -> ExecutedOperations {
    let priority_op = FullExit {
        account_id: AccountId(0),
        eth_address: Address::zero(),
        token: TokenId(0),
        is_legacy: false,
    };
    ExecutedOperations::PriorityOp(Box::new(ExecutedPriorityOp {
        priority_op: PriorityOp {
            serial_id: 0,
            data: ZkSyncPriorityOp::FullExit(priority_op.clone()),
            deadline_block: 0,
            eth_hash: H256::zero(),
            eth_block: 0,
            eth_block_index: None,
        },
        op: ZkSyncOp::FullExit(Box::new(FullExitOp {
            priority_op,
            withdraw_amount: None,
            creator_account_id: None,
            creator_address: None,
            serial_id: None,
            content_hash: None,
        })),
        block_index: 0,
        created_at: Utc::now(),
    }))
}

pub fn create_withdraw_tx() -> ExecutedOperations {
    let withdraw_op = ZkSyncOp::Withdraw(Box::new(WithdrawOp {
        tx: Withdraw::new(
            AccountId(0),
            Default::default(),
            Default::default(),
            TokenId(0),
            100u32.into(),
            10u32.into(),
            Nonce(12),
            Default::default(),
            None,
        ),
        account_id: AccountId(0),
    }));

    let executed_withdraw_op = ExecutedTx {
        signed_tx: withdraw_op.try_get_tx().unwrap().into(),
        success: true,
        op: Some(withdraw_op),
        fail_reason: None,
        block_index: None,
        created_at: Utc::now(),
        batch_id: None,
    };

    ExecutedOperations::Tx(Box::new(executed_withdraw_op))
}

pub fn create_change_pubkey_tx() -> ExecutedOperations {
    let sk = priv_key_from_fs(XorShiftRng::new_unseeded().gen());
    let pk = H256::random();

    let mut change_pub_key = ChangePubKey::new_signed(
        AccountId(1),
        Default::default(),
        PubKeyHash::from_privkey(&sk),
        TokenId(0),
        Default::default(),
        Default::default(),
        Default::default(),
        None,
        &sk,
        Some(ChainId(9)),
    )
    .unwrap();
    let domain = Eip712Domain::new(ChainId(9));
    let eth_signature = PackedEthSignature::sign_typed_data(&pk, &domain, &change_pub_key).unwrap();
    change_pub_key.eth_signature = Some(eth_signature.clone());
    change_pub_key.eth_auth_data = Some(ChangePubKeyEthAuthData::EIP712(ChangePubKeyEIP712Data {
        eth_signature,
        batch_hash: Default::default(),
    }));

    let change_pubkey_op = ZkSyncOp::ChangePubKeyOffchain(Box::new(ChangePubKeyOp {
        tx: change_pub_key,
        account_id: AccountId(0),
    }));

    let executed_change_pubkey_op = ExecutedTx {
        signed_tx: change_pubkey_op.try_get_tx().unwrap().into(),
        success: true,
        op: Some(change_pubkey_op),
        fail_reason: None,
        block_index: None,
        created_at: Utc::now(),
        batch_id: None,
    };

    ExecutedOperations::Tx(Box::new(executed_change_pubkey_op))
}
