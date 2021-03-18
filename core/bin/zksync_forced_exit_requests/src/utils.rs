use std::{convert::TryInto, ops::Sub};

use num::BigUint;
use num::FromPrimitive;
use zksync_crypto::ff::PrimeField;
pub use zksync_crypto::franklin_crypto::{eddsa::PrivateKey, jubjub::JubjubEngine};

pub use zksync_crypto::franklin_crypto::{
    alt_babyjubjub::fs::FsRepr,
    bellman::{pairing::bn256, PrimeFieldRepr},
};

pub type Engine = bn256::Bn256;

pub type Fs = <Engine as JubjubEngine>::Fs;

pub fn read_signing_key(private_key: &[u8]) -> anyhow::Result<PrivateKey<Engine>> {
    let mut fs_repr = FsRepr::default();
    fs_repr.read_be(private_key)?;
    Ok(PrivateKey::<Engine>(
        Fs::from_repr(fs_repr).expect("couldn't read private key from repr"),
    ))
}

pub fn extract_id_from_amount(amount: BigUint, digits_in_id: u32) -> (i64, BigUint) {
    let id_space_size: i64 = 10_i64.pow(digits_in_id);

    let id_space_size = BigUint::from_i64(id_space_size).unwrap();

    // Taking to the power of 1 and finding mod
    // is the only way to find mod of BigUint
    let one = BigUint::from_u8(1u8).unwrap();
    let id = amount.modpow(&one, &id_space_size);

    // After extracting the id we need to delete it
    // to make sure that amount is the same as in the db
    let amount = amount.sub(&id);

    (id.try_into().unwrap(), amount)
}

#[cfg(test)]
mod test {
    use std::ops::Add;
    use std::str::FromStr;

    use num::Zero;

    use super::*;

    fn test_extraction_for_id_amount(
        amount: BigUint,
        digits_in_id: u32,
        expected_id: i64,
        expected_amount: BigUint,
    ) {
        let (id, remain_amount) = extract_id_from_amount(amount, digits_in_id);

        assert_eq!(id, expected_id);
        assert_eq!(remain_amount, expected_amount);
    }

    #[test]
    fn test_extract_id_from_amount() {
        // Basic extraction
        test_extraction_for_id_amount(
            BigUint::from_str("12211").unwrap(),
            3,
            211,
            BigUint::from_str("12000").unwrap(),
        );

        // Note that there are not enough digits in the sent amount
        // Thus the amount should be equal to id
        test_extraction_for_id_amount(BigUint::from_str("11").unwrap(), 3, 11, BigUint::zero());

        // Here we test with some really large number, which could not possible
        // fit into 2^64
        let ten = BigUint::from_str("10").unwrap();
        let id: u32 = 211;
        let expected_amount = ten.pow(100);
        let amount = expected_amount.clone().add(id);
        test_extraction_for_id_amount(amount, 3, id.try_into().unwrap(), expected_amount);
    }
}
