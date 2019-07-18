#[macro_use]
extern crate log;

pub mod verification_key;
pub mod read_write_keys;
pub mod vk_contract_generator;

use verification_key::make_verification_key;

fn main() {
    env_logger::init();

    make_verification_key();
}
