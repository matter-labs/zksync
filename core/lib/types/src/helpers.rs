use num::{BigUint, FromPrimitive};
use zksync_crypto::params;
use zksync_crypto::primitives::FloatConversions;

use crate::{Account, AccountMap, AccountUpdates};

/// Given the account map, applies a sequence of updates to the state.
pub fn apply_updates(accounts: &mut AccountMap, updates: AccountUpdates) {
    for (id, update) in updates.into_iter() {
        let updated_account = Account::apply_update(accounts.remove(&id), update);
        if let Some(account) = updated_account {
            accounts.insert(id, account);
        }
    }
}

/// Replaces a sequence of updates with the sequence of updates required to revert
/// the applied state change.
pub fn reverse_updates(updates: &mut AccountUpdates) {
    updates.reverse();
    for (_, acc_upd) in updates.iter_mut() {
        *acc_upd = acc_upd.reversed_update();
    }
}

/// Transforms the token amount into packed form.
/// If the provided token amount is not packable, it is rounded down to the
/// closest amount that fits in packed form. As a result, some precision will be lost.
pub fn pack_token_amount(amount: &BigUint) -> Vec<u8> {
    FloatConversions::pack(
        amount,
        params::AMOUNT_EXPONENT_BIT_WIDTH,
        params::AMOUNT_MANTISSA_BIT_WIDTH,
    )
}

/// Transforms the fee amount into the packed form.
/// As the packed form for fee is smaller than one for the token,
/// the same value must be packable as a token amount, but not packable
/// as a fee amount.
/// If the provided fee amount is not packable, it is rounded down to the
/// closest amount that fits in packed form. As a result, some precision will be lost.
pub fn pack_fee_amount(amount: &BigUint) -> Vec<u8> {
    FloatConversions::pack(
        amount,
        params::FEE_EXPONENT_BIT_WIDTH,
        params::FEE_MANTISSA_BIT_WIDTH,
    )
}

/// Checks whether the token amount can be packed (and thus used in the transaction).
pub fn is_token_amount_packable(amount: &BigUint) -> bool {
    Some(amount.clone()) == unpack_token_amount(&pack_token_amount(amount))
}

/// Checks whether the fee amount can be packed (and thus used in the transaction).
pub fn is_fee_amount_packable(amount: &BigUint) -> bool {
    Some(amount.clone()) == unpack_fee_amount(&pack_fee_amount(amount))
}

/// Attempts to unpack the token amount.
pub fn unpack_token_amount(data: &[u8]) -> Option<BigUint> {
    FloatConversions::unpack(
        data,
        params::AMOUNT_EXPONENT_BIT_WIDTH,
        params::AMOUNT_MANTISSA_BIT_WIDTH,
    )
    .and_then(BigUint::from_u128)
}

/// Attempts to unpack the fee amount.
pub fn unpack_fee_amount(data: &[u8]) -> Option<BigUint> {
    FloatConversions::unpack(
        data,
        params::FEE_EXPONENT_BIT_WIDTH,
        params::FEE_MANTISSA_BIT_WIDTH,
    )
    .and_then(BigUint::from_u128)
}

/// Returns the closest possible packable token amount.
/// Returned amount is always less or equal to the provided amount.
pub fn closest_packable_fee_amount(amount: &BigUint) -> BigUint {
    let fee_packed = pack_fee_amount(&amount);
    unpack_fee_amount(&fee_packed).expect("fee repacking")
}

/// Returns the closest possible packable fee amount.
/// Returned amount is always less or equal to the provided amount.
pub fn closest_packable_token_amount(amount: &BigUint) -> BigUint {
    let fee_packed = pack_token_amount(&amount);
    unpack_token_amount(&fee_packed).expect("token amount repacking")
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::TokenLike;
    use serde::{Deserialize, Serialize};

    #[test]
    fn test_roundtrip() {
        let zero = BigUint::from_u32(1).unwrap();
        let one = BigUint::from_u32(1).unwrap();
        {
            let round_trip_zero = unpack_token_amount(&pack_token_amount(&zero));
            let round_trip_one = unpack_token_amount(&pack_token_amount(&one));
            assert_eq!(Some(zero.clone()), round_trip_zero);
            assert_eq!(Some(one.clone()), round_trip_one);
        }
        {
            let round_trip_zero = unpack_fee_amount(&pack_fee_amount(&zero));
            let round_trip_one = unpack_fee_amount(&pack_fee_amount(&one));
            assert_eq!(Some(zero), round_trip_zero);
            assert_eq!(Some(one), round_trip_one);
        }
    }

    #[test]
    fn detect_unpackable() {
        let max_mantissa_token =
            BigUint::from_u128((1u128 << params::AMOUNT_MANTISSA_BIT_WIDTH) - 1).unwrap();
        let max_mantissa_fee =
            BigUint::from_u128((1u128 << params::FEE_MANTISSA_BIT_WIDTH) - 1).unwrap();
        assert!(is_token_amount_packable(&max_mantissa_token));
        assert!(is_fee_amount_packable(&max_mantissa_fee));
        assert!(!is_token_amount_packable(
            &(max_mantissa_token + BigUint::from(1u32))
        ));
        assert!(!is_fee_amount_packable(
            &(max_mantissa_fee + BigUint::from(1u32))
        ));
    }

    #[test]
    fn pack_to_closest_packable() {
        let fee = BigUint::from(1_234_123_424u32);
        assert!(
            !is_fee_amount_packable(&fee),
            "fee should not be packable for this test"
        );
        let closest_packable_fee = closest_packable_fee_amount(&fee);
        assert!(
            is_fee_amount_packable(&closest_packable_fee),
            "repacked fee should be packable"
        );
        assert_ne!(
            closest_packable_fee,
            BigUint::from(0u32),
            "repacked fee should not be 0"
        );
        assert!(
            closest_packable_fee < fee,
            "packable fee should be less then original"
        );
        println!(
            "fee: original: {}, truncated: {}",
            fee, closest_packable_fee
        );

        let token = BigUint::from(123_456_789_123_456_789u64);
        assert!(
            !is_token_amount_packable(&token),
            "token should not be packable for this test"
        );
        let closest_packable_token = closest_packable_token_amount(&token);
        assert!(
            is_token_amount_packable(&closest_packable_token),
            "repacked token amount should be packable"
        );
        assert_ne!(
            closest_packable_token,
            BigUint::from(0u32),
            "repacked token should not be 0"
        );
        assert!(
            closest_packable_token < token,
            "packable token should be less then original"
        );
        println!(
            "token: original: {}, packable: {}",
            token, closest_packable_token
        );
    }

    #[test]
    fn token_like_serialization() {
        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct Query {
            token: TokenLike,
        }
        let test_cases = vec![
            (
                Query {
                    token: TokenLike::Address(
                        "c919467ee96806d584cae8d0b11504b26fedfbab".parse().unwrap(),
                    ),
                },
                r#"{"token":"0xc919467ee96806d584cae8d0b11504b26fedfbab"}"#,
            ),
            (
                Query {
                    token: TokenLike::Symbol("ETH".to_string()),
                },
                r#"{"token":"ETH"}"#,
            ),
            (
                Query {
                    token: TokenLike::Id(14),
                },
                r#"{"token":14}"#,
            ),
        ];

        for (query, json_str) in test_cases {
            let ser = serde_json::to_string(&query).expect("ser");
            assert_eq!(ser, json_str);
            let de = serde_json::from_str(&ser).expect("de");
            assert_eq!(query, de);
        }
    }
}
