use crate::primitives::{GetBits, GetBitsFixed};
use bigdecimal::BigDecimal;
use super::circuit;

use super::{Engine, Fr, FieldBytes};

#[derive(Debug, Clone, Default, Queryable, Serialize, Deserialize)]
pub struct Account {
    pub balance:    BigDecimal,
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

#[test]
fn test_default_account() {
    let a = Account::default();
    println!("{:?}", a);

    use ff::PrimeField;
    println!("{:?}", &a.balance.to_string());
    println!("{:?}", &a.nonce.to_string());
    println!("{:?}", &a.pub_x.into_repr().to_string());
    println!("{:?}", &a.pub_y.into_repr().to_string());

    let i: u128 = 5;
    let v = serde_json::to_value(&i);
    println!("vi = {:?}", &v);

    let v = serde_json::to_value(&a);
    println!("v = {:?}", &v);

    let d: circuit::Account<Engine> = serde_json::from_value(v.unwrap()).unwrap();
    println!("d = {:?}", &d);

    let ca = circuit::Account::<Engine>::from(a.clone());
    println!("ca = {:?}", ca);

    let bits = a.get_bits_le();
    println!("{:?}", bits);
}