use zksync_circuit::witness::utils::fr_from_bytes;

use crate::account::read_accounts;
use crate::hasher::{get_state_root_hash, BALANCE_TREE_11};

#[test]
fn test_sample_tree_hashing() {
    let accounts = read_accounts(
        String::from("./sample/accounts"),
        String::from("./sample/balances"),
    )
    .unwrap();

    let expected_hash_str = "2bd61f42837c0fa77fc113b3b341c520edb1ffadefc48c2b907901aaaf42b906";
    let expected_hash_bytes = hex::decode(expected_hash_str).unwrap();
    let expected_hash = fr_from_bytes(expected_hash_bytes);

    let hash_11 = get_state_root_hash(&accounts, &BALANCE_TREE_11);

    assert_eq!(
        hash_11, expected_hash,
        "The recalculated hash is not equal to the current one."
    );
}
