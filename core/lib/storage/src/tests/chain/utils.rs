// External imports

use parity_crypto::publickey::{Generator, Random};
use zksync_basic_types::Address;
// Workspace imports
use num::BigUint;
use std::ops::Deref;
use zksync_crypto::rand::Rng;
use zksync_crypto::Fr;
use zksync_types::tx::{EthSignData, PackedEthSignature, TxEthSignature};
use zksync_types::{
    Action, Operation,
    {
        block::{Block, ExecutedOperations},
        AccountUpdate, BlockNumber, PubKeyHash,
    },
};
// Local imports

pub fn acc_create_random_updates<R: Rng>(
    rng: &mut R,
) -> impl Iterator<Item = (u32, AccountUpdate)> {
    let id: u32 = rng.gen();
    let balance = u128::from(rng.gen::<u64>());
    let nonce: u32 = rng.gen();
    let pub_key_hash = PubKeyHash { data: rng.gen() };
    let address: Address = rng.gen::<[u8; 20]>().into();

    let mut a = zksync_types::account::Account::default_with_address(&address);
    let old_nonce = nonce;
    a.nonce = old_nonce + 2;
    a.pub_key_hash = pub_key_hash;

    let old_balance = a.get_balance(0);
    a.set_balance(0, BigUint::from(balance));
    let new_balance = a.get_balance(0);
    vec![
        (
            id,
            AccountUpdate::Create {
                nonce: old_nonce,
                address: a.address,
            },
        ),
        (
            id,
            AccountUpdate::ChangePubKeyHash {
                old_nonce,
                old_pub_key_hash: PubKeyHash::default(),
                new_nonce: old_nonce + 1,
                new_pub_key_hash: a.pub_key_hash,
            },
        ),
        (
            id,
            AccountUpdate::UpdateBalance {
                old_nonce: old_nonce + 1,
                new_nonce: old_nonce + 2,
                balance_update: (0, old_balance, new_balance),
            },
        ),
    ]
    .into_iter()
}

pub fn get_operation(block_number: BlockNumber, action: Action, block_size: usize) -> Operation {
    Operation {
        id: None,
        action,
        block: Block::new(
            block_number,
            Fr::default(),
            0,
            Vec::new(),
            (0, 0),
            block_size,
            1_000_000.into(),
            1_500_000.into(),
        ),
    }
}

pub fn get_operation_with_txs(
    block_number: BlockNumber,
    action: Action,
    block_size: usize,
    txs: Vec<ExecutedOperations>,
) -> Operation {
    Operation {
        id: None,
        action,
        block: Block::new(
            block_number,
            Fr::default(),
            0,
            txs,
            (0, 0),
            block_size,
            1_000_000.into(),
            1_500_000.into(),
        ),
    }
}

/// Generates EthSignData for testing (not a valid signature)
pub fn get_eth_sing_data(message: String) -> EthSignData {
    let keypair = Random.generate();
    let private_key = keypair.secret();

    let signature = PackedEthSignature::sign(private_key.deref(), &message.as_bytes()).unwrap();

    EthSignData {
        message,
        signature: TxEthSignature::EthereumSignature(signature),
    }
}
