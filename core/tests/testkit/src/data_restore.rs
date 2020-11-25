use web3::{transports::Http, types::Address};

use zksync_crypto::Fr;
use zksync_data_restore::{
    data_restore_driver::DataRestoreDriver, inmemory_storage_interactor::InMemoryStorageInteractor,
    ETH_BLOCKS_STEP,
};
use zksync_types::AccountMap;

use crate::external_commands::Contracts;

pub async fn verify_restore(
    web3_url: &str,
    available_block_chunk_sizes: Vec<usize>,
    contracts: &Contracts,
    fee_account_address: Address,
    acc_state_from_test_setup: AccountMap,
    tokens: Vec<u16>,
    root_hash: Fr,
) {
    let transport = Http::new(web3_url).expect("http transport start");

    let mut interactor = InMemoryStorageInteractor::new();
    let mut driver = DataRestoreDriver::new(
        transport,
        contracts.governance,
        contracts.contract,
        ETH_BLOCKS_STEP,
        0,
        available_block_chunk_sizes,
        true,
        Default::default(),
    );

    interactor.insert_new_account(0, &fee_account_address);
    driver.load_state_from_storage(&mut interactor).await;
    driver.run_state_update(&mut interactor).await;

    assert_eq!(driver.tree_state.root_hash(), root_hash);

    for (id, account) in acc_state_from_test_setup {
        let driver_acc = driver.tree_state.get_account(id).expect("Should exist");
        let inter_acc = interactor.get_account(&id).expect("Should exist");
        for id in &tokens {
            assert_eq!(driver_acc.address, inter_acc.address);
            assert_eq!(account.address, inter_acc.address);
            assert_eq!(driver_acc.get_balance(*id), account.get_balance(*id));
            assert_eq!(inter_acc.get_balance(*id), account.get_balance(*id));
        }
    }
    println!("Data restore test is ok")
}
