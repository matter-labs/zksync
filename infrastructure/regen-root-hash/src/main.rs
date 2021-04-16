mod account;
mod hasher;

use zksync_circuit::witness::utils::fr_from_bytes;

use account::read_accounts;
use hasher::{get_state_root_hash, BALANCE_TREE_11, BALANCE_TREE_32};

fn main() {
    let accounts = read_accounts(
        String::from("./sample/accounts"),
        String::from("./sample/balances"),
    )
    .unwrap();

    let current_hash_str = "2bd61f42837c0fa77fc113b3b341c520edb1ffadefc48c2b907901aaaf42b906";
    let current_hash_bytes = hex::decode(current_hash_str).unwrap();
    let current_hash = fr_from_bytes(current_hash_bytes);

    let hash_11 = get_state_root_hash(&accounts, &BALANCE_TREE_11);

    assert_eq!(
        hash_11, current_hash,
        "The recalculated hash is not equal to the current one."
    );

    print!("hash11: {} \n", hash_11.to_string());

    let hash_32 = get_state_root_hash(&accounts, &BALANCE_TREE_32);

    print!("hash32: {} \n", hash_32.to_string());
}
