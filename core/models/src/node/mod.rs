use super::merkle_tree::{PedersenHasher, SparseMerkleTree};
use super::params;
use super::primitives::pack_as_float;
use bigdecimal::BigDecimal;
use failure::bail;
use pairing::bn256;

pub mod account;
pub mod block;
pub mod config;
pub mod operations;
pub mod priority_ops;
pub mod tx;

pub use web3::types::{H256, U128, U256};

pub use self::account::{Account, AccountAddress, AccountUpdate};
pub use self::operations::{DepositOp, FranklinOp, TransferOp, TransferToNewOp, WithdrawOp};
pub use self::priority_ops::{Deposit, FranklinPriorityOp, FullExit};
pub use self::tx::{Close, FranklinTx, Transfer, Withdraw};

pub type Engine = bn256::Bn256;
pub type Fr = bn256::Fr;

pub type AccountMap = fnv::FnvHashMap<u32, Account>;
pub type AccountUpdates = Vec<(u32, AccountUpdate)>;
pub type AccountTree = SparseMerkleTree<Account, Fr, PedersenHasher<Engine>>;

pub fn apply_updates(accounts: &mut AccountMap, updates: AccountUpdates) {
    for (id, update) in updates.into_iter() {
        let updated_account = Account::apply_update(accounts.remove(&id), update);
        if let Some(account) = updated_account {
            accounts.insert(id, account);
        }
    }
}

pub fn reverse_updates(updates: &mut AccountUpdates) {
    updates.reverse();
    for (_, acc_upd) in updates.iter_mut() {
        *acc_upd = acc_upd.reversed_update();
    }
}

pub type TokenId = u16;

/// 3 bytes used.
pub type AccountId = u32;
pub type BlockNumber = u32;
pub type Nonce = u32;

pub fn pack_token_amount(amount: &BigDecimal) -> Vec<u8> {
    pack_as_float(
        amount,
        params::AMOUNT_EXPONENT_BIT_WIDTH,
        params::AMOUNT_MANTISSA_BIT_WIDTH,
    )
}

pub fn pack_fee_amount(amount: &BigDecimal) -> Vec<u8> {
    pack_as_float(
        amount,
        params::FEE_EXPONENT_BIT_WIDTH,
        params::FEE_MANTISSA_BIT_WIDTH,
    )
}

pub fn convert_to_float(
    integer: u128,
    exponent_length: usize,
    mantissa_length: usize,
    exponent_base: u32,
) -> Result<Vec<bool>, failure::Error> {
    unimplemented!("proper convert to float");
    //    let integer = BigDecimal::(integer);
    //    let exponent_base = BigDecimal::from(exponent_base);
    //    let mut max_exponent = BigDecimal::from(1u128);
    //    let max_power = (1 << exponent_length) - 1;
    //
    //    for _ in 0..max_power
    //        {
    //            max_exponent = max_exponent * exponent_base;
    //        }
    //
    //    let max_mantissa = BigDecimal::from((1u128 << mantissa_length) - 1);
    //
    //    if BigDecimal::from(integer) > (max_mantissa * max_exponent) {
    //        bail!("Integer is too big");
    //    }
    //
    //    let mut exponent: usize = 0;
    //    let mut mantissa = integer;
    //
    //    if BigDecimal::from(integer) > max_mantissa.to_u128() {
    //        // always try best precision
    //        let exponent_guess = integer / max_mantissa;
    //        let mut exponent_temp = exponent_guess;
    //
    //        loop {
    //            if exponent_temp < exponent_base {
    //                break
    //            }
    //            exponent_temp = exponent_temp / exponent_base;
    //            exponent += 1;
    //        }
    //
    //        exponent_temp = 1u128;
    //        for _ in 0..exponent
    //            {
    //                exponent_temp = exponent_temp * exponent_base;
    //            }
    //
    //        if exponent_temp * max_mantissa < integer
    //        {
    //            exponent += 1;
    //            exponent_temp = exponent_temp * exponent_base;
    //        }
    //
    //        mantissa = integer / exponent_temp;
    //    }
    //
    //    // encode into bits. First bits of mantissa in LE order
    //
    //    let mut encoding = vec![];
    //
    //    for i in 0..exponent_length {
    //        if exponent & (1 << i) != 0 {
    //            encoding.extend(&[true; 1]);
    //        } else {
    //            encoding.extend(&[false; 1]);
    //        }
    //    }
    //
    //    for i in 0..mantissa_length {
    //        if mantissa & (1 << i) != 0 {
    //            encoding.extend(&[true; 1]);
    //        } else {
    //            encoding.extend(&[false; 1]);
    //        }
    //    }
    //
    //    assert!(encoding.len() == exponent_length + mantissa_length);
    //
    //    Ok(encoding)
}

#[cfg(test)]
mod test {
    use super::*;
    use bigdecimal::BigDecimal;
    #[test]
    fn test_pack() {
        //        println!("{:x?}", pack_token_amount(&BigDecimal::from(2)));
        println!("{:x?}", pack_fee_amount(&BigDecimal::from(1)));
    }
}
