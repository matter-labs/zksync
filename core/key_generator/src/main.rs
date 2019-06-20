#[macro_use]
extern crate log;

pub mod depositor_key;
pub mod exitor_key;
pub mod read_write_keys;
pub mod transactor_key;
pub mod vk_contract_generator;

use depositor_key::make_depositor_key;
use exitor_key::make_exitor_key;
use transactor_key::make_transactor_key;

fn main() {
    env_logger::init();

    make_depositor_key();
    make_exitor_key();
    make_transactor_key();
}
