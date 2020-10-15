use bigdecimal::BigDecimal;
use num::{
    bigint::ToBigInt,
    rational::Ratio,
    traits::{sign::Signed, Pow},
    BigUint,
};

pub fn ratio_to_big_decimal(num: &Ratio<BigUint>, precision: usize) -> BigDecimal {
    let bigint = round_precision_raw_no_div(num, precision)
        .to_bigint()
        .unwrap();
    BigDecimal::new(bigint, precision as i64)
}

pub fn big_decimal_to_ratio(num: &BigDecimal) -> Result<Ratio<BigUint>, anyhow::Error> {
    let (big_int, exp) = num.as_bigint_and_exponent();
    anyhow::ensure!(!big_int.is_negative(), "BigDecimal should be unsigned");
    let big_uint = big_int.to_biguint().unwrap();
    let ten_pow = BigUint::from(10 as u32).pow(exp as u128);
    Ok(Ratio::new(big_uint, ten_pow))
}

fn round_precision_raw_no_div(num: &Ratio<BigUint>, precision: usize) -> BigUint {
    let ten_pow = BigUint::from(10u32).pow(precision);
    (num * ten_pow).round().to_integer()
}

pub fn round_precision(num: &Ratio<BigUint>, precision: usize) -> Ratio<BigUint> {
    let ten_pow = BigUint::from(10u32).pow(precision);
    let numerator = (num * &ten_pow).trunc().to_integer();
    Ratio::new(numerator, ten_pow)
}
