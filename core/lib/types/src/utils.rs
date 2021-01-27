//! Utilities used in tx module.

// External uses.
use num::{BigUint, Zero};
use serde::{
    de::{value::SeqAccessDeserializer, Error, SeqAccess, Visitor},
    Deserialize, Deserializer,
};

// Workspace uses.
use zksync_utils::format_units;

// Local uses.
use crate::Address;

/// Deserializes either a `String` or `Vec<u8>` into `Vec<u8>`.
/// The reason we cannot expect just a vector is backward compatibility: messages
/// used to be stored as strings.
pub fn deserialize_eth_message<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: Deserializer<'de>,
{
    struct StringOrVec;

    impl<'de> Visitor<'de> for StringOrVec {
        type Value = Vec<u8>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a byte array or a string")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: Error,
        {
            Ok(v.as_bytes().to_vec())
        }

        fn visit_seq<A>(self, seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            Deserialize::deserialize(SeqAccessDeserializer::new(seq))
        }
    }

    deserializer.deserialize_any(StringOrVec)
}

/// Serialize `H256` as `Vec<u8>`.
///
/// This workaround used for backward compatibility
/// with the old serialize/deserialize behaviour of the fields
/// whose type changed from `Vec<u8>` to `H256`.
pub mod h256_as_vec {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::iter;
    use zksync_basic_types::H256;

    pub fn serialize<S>(val: &H256, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let val = val.as_bytes().to_vec();
        val.serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<H256, D::Error>
    where
        D: Deserializer<'de>,
    {
        let expected_size = H256::len_bytes();

        let mut val = Vec::deserialize(deserializer)?;
        if let Some(padding_size) = expected_size.checked_sub(val.len()) {
            if padding_size > 0 {
                val = iter::repeat(0).take(padding_size).chain(val).collect();
            }
        }

        Ok(H256::from_slice(&val))
    }
}

/// Construct the first part of the message that should be signed by Ethereum key.
/// The pattern is as follows:
///
/// [{Transfer/Withdraw} {amount} {token} to: {to_address}]
/// [Fee: {fee} {token}]
///
/// Note that both lines are optional.
pub fn ethereum_sign_message_part(
    transaction: &str,
    token_symbol: &str,
    decimals: u8,
    amount: &BigUint,
    fee: &BigUint,
    to: &Address,
) -> String {
    let mut message = if !amount.is_zero() {
        format!(
            "{transaction} {amount} {token} to: {to:?}",
            transaction = transaction,
            amount = format_units(amount, decimals),
            token = token_symbol,
            to = to
        )
    } else {
        String::new()
    };
    if !fee.is_zero() {
        if !message.is_empty() {
            message.push('\n');
        }
        message.push_str(
            format!(
                "Fee: {fee} {token}",
                fee = format_units(fee, decimals),
                token = token_symbol
            )
            .as_str(),
        );
    }
    message
}
