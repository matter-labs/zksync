use sapling_crypto::alt_babyjubjub::{JubjubEngine};

use super::account::Account;

pub trait State<E: JubjubEngine> {  
    fn get_accounts(&self) -> Vec<(u32, Account<E>)>;
    fn block_number(&self) -> u32;
    fn root_hash(&self) -> E::Fr;
}
