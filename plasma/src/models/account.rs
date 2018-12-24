use crate::primitives::{GetBits};
use bigdecimal::BigDecimal;
use super::circuit;

use super::{Engine, Fr};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub balance:    BigDecimal,
    pub nonce:      u32,
    pub public_key_x: Fr,
    pub public_key_y: Fr,
}

impl GetBits for Account {
    fn get_bits_le(&self) -> Vec<bool> {

        circuit::Account::<Engine>::from(self.clone()).get_bits_le()
        
        // TODO: make more efficient:

        // let mut leaf_content = Vec::new();
        // leaf_content.extend(self.balance.get_bits_le_fixed(params::BALANCE_BIT_WIDTH));
        // leaf_content.extend(self.nonce.get_bits_le_fixed(params::NONCE_BIT_WIDTH));
        // leaf_content.extend(self.pub_x.get_bits_le_fixed(params::FR_BIT_WIDTH));
        // leaf_content.extend(self.pub_y.get_bits_le_fixed(params::FR_BIT_WIDTH));
        // leaf_content
    }
}

#[test]
fn test_default_account() {
    let a = Account::default();
    a.get_bits_le();
}