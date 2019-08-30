#[macro_use]
extern crate log;

pub mod franklin_key;
pub mod vk_contract_generator;

use franklin_key::make_franklin_key;

fn main() {
    env_logger::init();

    make_franklin_key();
}
