// External imports
use web3::types::Address;
// Workspace imports
use crypto_exports::rand::Rng;
use models::{
    node::{block::Block, AccountUpdate, BlockNumber, Fr, PubKeyHash},
    primitives::u128_to_bigdecimal,
    Action, Operation,
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

    let mut a = models::node::account::Account::default_with_address(&address);
    let old_nonce = nonce;
    a.nonce = old_nonce + 2;
    a.pub_key_hash = pub_key_hash;

    let old_balance = a.get_balance(0);
    a.set_balance(0, u128_to_bigdecimal(balance));
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

pub fn get_operation(
    block_number: BlockNumber,
    action: Action,
    accounts_updated: Vec<(u32, AccountUpdate)>,
) -> Operation {
    Operation {
        id: None,
        action,
        block: Block::new(block_number, Fr::default(), 0, Vec::new(), (0, 0), 100),
        accounts_updated,
    }
}
