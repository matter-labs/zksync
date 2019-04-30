#![allow(unused_must_use)]
#![allow(deprecated)]

extern crate web3;
extern crate tiny_keccak;
extern crate tokio_core;
extern crate ethabi;

pub mod events;
pub mod franklin_transaction;
pub mod blocks;
pub mod helpers;

#[cfg(test)]
mod test {

    use super::*;
    use web3::types::U256;
    use tokio_core::reactor::Core;

    #[test]
    fn test_events() {
        let mut events = events::EventsFranklin::get_past_state_with_blocks_delta(helpers::InfuraEndpoint::Rinkeby, U256::from(2404)).unwrap();
        println!("Committed old: {:?}", events.committed_blocks);
        println!("Verified old: {:?}", events.verified_blocks);
        let mut eloop = Core::new().unwrap();
        events.make_new_sorted_logs_subscription(&mut eloop);
    }
}