use crate::utils::WitnessTestAccount;
use change_pubkey_offchain::change_pubkey_offchain_witness_benches;
use close_account::close_account_witness_benches;
use criterion::criterion_main;
use deposit::deposit_witness_benches;
use forced_exit::forced_exit_benches;
use full_exit::full_exit_benches;
use transfer::transfer_benches;
use transfer_to_new::transfer_to_new_benches;
use withdraw::withdraw_benches;
use zksync_types::AccountId;

mod change_pubkey_offchain;
mod close_account;
mod deposit;
mod forced_exit;
mod full_exit;
mod transfer;
mod transfer_to_new;
mod utils;
mod withdraw;

fn generate_accounts(count: usize) -> Vec<WitnessTestAccount> {
    let mut accounts: Vec<WitnessTestAccount> = Vec::new();
    for i in 0..count {
        accounts.push(WitnessTestAccount::new(AccountId((i + 1) as u32), 200u64));
    }
    accounts
}

criterion_main!(
    change_pubkey_offchain_witness_benches,
    close_account_witness_benches,
    deposit_witness_benches,
    forced_exit_benches,
    full_exit_benches,
    transfer_to_new_benches,
    transfer_benches,
    withdraw_benches,
);
