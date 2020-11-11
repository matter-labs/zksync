use num::BigUint;
use zksync_crypto::{
    rand::{Rng, SeedableRng, XorShiftRng},
    Fr,
};
use zksync_types::helpers::apply_updates;
use zksync_types::{AccountMap, AccountUpdate, Action, ActionType, Address, Operation, PubKeyHash};

use crate::chain::block::BlockSchema;
use crate::chain::operations::records::NewOperation;
use crate::chain::operations::OperationsSchema;
use crate::chain::state::StateSchema;
use crate::data_restore::records::NewBlockEvent;
use crate::data_restore::DataRestoreSchema;
use crate::StorageProcessor;
use zksync_types::block::Block;

pub fn create_rng() -> XorShiftRng {
    XorShiftRng::from_seed([0, 1, 2, 3])
}

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

pub fn apply_random_updates(
    mut accounts: AccountMap,
    rng: &mut XorShiftRng,
) -> (AccountMap, Vec<(u32, AccountUpdate)>) {
    let updates = {
        let mut updates = Vec::new();
        updates.extend(acc_create_random_updates(rng));
        updates.extend(acc_create_random_updates(rng));
        updates.extend(acc_create_random_updates(rng));
        updates
    };
    apply_updates(&mut accounts, updates.clone());
    (accounts, updates)
}

pub async fn create_data_restore_data(storage: &mut StorageProcessor<'_>) {
    sqlx::query!(
        "INSERT INTO data_restore_storage_state_update (storage_state) VALUES ($1)",
        "None",
    )
    .execute(storage.conn())
    .await
    .unwrap();
    let mut events = vec![];
    for i in 0..10 {
        events.push(NewBlockEvent {
            block_type: "Committed".to_string(),
            transaction_hash: [10; 32].to_vec(),
            block_num: i as i64,
        })
    }
    for i in 0..10 {
        events.push(NewBlockEvent {
            block_type: "Verified".to_string(),
            transaction_hash: [10; 32].to_vec(),
            block_num: i as i64,
        })
    }

    for event in events.iter() {
        sqlx::query!(
                "INSERT INTO data_restore_events_state (block_type, transaction_hash, block_num) VALUES ($1, $2, $3)",
                event.block_type, event.transaction_hash, event.block_num
            )
			.execute(storage.conn())
			.await.unwrap();
    }
    let mut rng = create_rng();
    sqlx::query!(
        "
        INSERT INTO tokens
        VALUES (0,
                '0x0000000000000000000000000000000000000000',
                'ETH',
                18)
        "
    )
    .execute(storage.conn())
    .await
    .unwrap();

    // Create the input data for three blocks.
    // Data for the next block is based on previous block data.
    let (accounts_block_1, updates_block_1) = apply_random_updates(AccountMap::default(), &mut rng);
    let (accounts_block_2, updates_block_2) =
        apply_random_updates(accounts_block_1.clone(), &mut rng);
    let (_accounts_block_3, updates_block_3) =
        apply_random_updates(accounts_block_2.clone(), &mut rng);
    DataRestoreSchema(storage)
        .update_last_watched_block_number("0")
        .await
        .unwrap();

    // Store the states in schema.
    StateSchema(storage)
        .commit_state_update(1, &updates_block_1, 0)
        .await
        .unwrap();

    StateSchema(storage).apply_state_update(1).await.unwrap();
    StateSchema(storage)
        .commit_state_update(2, &updates_block_2, 0)
        .await
        .unwrap();
    StateSchema(storage).apply_state_update(2).await.unwrap();
    StateSchema(storage)
        .commit_state_update(3, &updates_block_3, 0)
        .await
        .unwrap();
    StateSchema(storage).apply_state_update(3).await.unwrap();

    BlockSchema(storage)
        .execute_operation(Operation {
            id: None,
            action: Action::Commit,
            block: Block::new(
                0,
                Fr::default(),
                3,
                Vec::new(),
                (0, 0),
                100,
                1_000_000.into(),
                1_500_000.into(),
            ),
        })
        .await
        .unwrap();

    // We have to store the operations as well (and for verify below too).
    for block_number in 1..=3 {
        OperationsSchema(storage)
            .store_operation(NewOperation {
                block_number,
                action_type: ActionType::COMMIT.to_string(),
            })
            .await
            .unwrap();
    }
}
