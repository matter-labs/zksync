use web3::{transports::Http, types::Address, Web3};

use zksync_crypto::Fr;
use zksync_data_restore::{
    data_restore_driver::DataRestoreDriver, inmemory_storage_interactor::InMemoryStorageInteractor,
    ETH_BLOCKS_STEP,
};
use zksync_types::{AccountId, AccountMap, TokenId};

use crate::{external_commands::Contracts, TestkitConfig};

use zksync_data_restore::contract::ZkSyncDeployedContract;
use zksync_data_restore::storage_interactor::StorageInteractor;

pub async fn verify_restore(
    testkit_config: &TestkitConfig,
    contracts: &Contracts,
    fee_account_address: Address,
    acc_state_from_test_setup: AccountMap,
    tokens: Vec<TokenId>,
    root_hash: Fr,
) {
    let web3 = Web3::new(Http::new(&testkit_config.web3_url).expect("http transport start"));

    let contract = ZkSyncDeployedContract::version4(web3.eth(), contracts.contract);
    let mut driver = DataRestoreDriver::new(
        web3,
        contracts.governance,
        testkit_config.contract_upgrade_eth_blocks.clone(),
        testkit_config.init_contract_version,
        ETH_BLOCKS_STEP,
        0,
        true,
        Default::default(),
        contract,
    );

    let mut db = InMemoryStorageInteractor::new();

    db.insert_new_account(AccountId(0), &fee_account_address);

    let mut interactor = StorageInteractor::InMemory(db);
    driver.load_state_from_storage(&mut interactor).await;
    driver.run_state_update(&mut interactor).await;

    assert_eq!(driver.tree_state.root_hash(), root_hash);

    let db = match &mut interactor {
        StorageInteractor::InMemory(db) => db,
        _ => unreachable!(),
    };

    for (id, account) in acc_state_from_test_setup {
        let driver_acc = driver.tree_state.get_account(id).expect("Should exist");
        let inter_acc = db.get_account(&id).expect("Should exist");
        for id in &tokens {
            assert_eq!(driver_acc.address, inter_acc.address);
            assert_eq!(account.address, inter_acc.address);
            assert_eq!(driver_acc.get_balance(*id), account.get_balance(*id));
            assert_eq!(inter_acc.get_balance(*id), account.get_balance(*id));
        }
    }
    println!("Data restore test is ok")
}
