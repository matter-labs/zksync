#[macro_use]
extern crate log;

pub mod depositor_key;
pub mod vk_contract_generator;

use depositor_key::make_franklin_key;

fn main() {
    env_logger::init();

    make_franklin_key();
}
