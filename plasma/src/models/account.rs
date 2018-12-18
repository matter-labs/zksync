use crate::primitives::{GetBits, GetBitsFixed};
use bigdecimal::BigDecimal;
use super::circuit;

use super::{Engine, Fr, FieldBytes};

#[derive(Debug, Clone, Default, Queryable, Serialize, Deserialize)]
pub struct Account {
    pub balance:    u128,
    pub nonce:      u32,
    pub pub_x:      FieldBytes,
    pub pub_y:      FieldBytes,
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
