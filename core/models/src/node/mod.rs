use super::merkle_tree::{PedersenHasher, SparseMerkleTree};
use super::params;
use super::primitives::{pack_as_float, u128_to_bigdecimal, unpack_float};
use crate::franklin_crypto::bellman::pairing::bn256;
use crate::franklin_crypto::{
    eddsa::{PrivateKey as PrivateKeyImport, PublicKey as PublicKeyImport},
    jubjub::JubjubEngine,
};
use bigdecimal::BigDecimal;

pub mod account;
pub mod block;
pub mod config;
pub mod operations;
pub mod priority_ops;
pub mod tx;

pub use web3::types::{H256, U128, U256};

pub use self::account::{Account, AccountUpdate, PubKeyHash};
pub use self::block::{ExecutedOperations, ExecutedPriorityOp, ExecutedTx};
pub use self::operations::{
    CloseOp, DepositOp, FranklinOp, FullExitOp, TransferOp, TransferToNewOp, WithdrawOp,
};
pub use self::priority_ops::{Deposit, FranklinPriorityOp, FullExit, PriorityOp};
pub use self::tx::{Close, FranklinTx, Transfer, Withdraw};

pub type Engine = bn256::Bn256;
pub type Fr = bn256::Fr;
pub type Fs = <Engine as JubjubEngine>::Fs;

pub type AccountMap = fnv::FnvHashMap<u32, Account>;
pub type AccountUpdates = Vec<(u32, AccountUpdate)>;
pub type AccountTree = SparseMerkleTree<Account, Fr, PedersenHasher<Engine>>;

pub type PrivateKey = PrivateKeyImport<Engine>;
pub type PublicKey = PublicKeyImport<Engine>;
pub type Address = web3::types::Address;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
/// Order of the fields are important (from more specific types to less specific types)
pub enum TokenLike {
    Id(TokenId),
    Address(Address),
    Symbol(String),
}

impl From<TokenId> for TokenLike {
    fn from(id: TokenId) -> Self {
        Self::Id(id)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
/// Token supported in zkSync protocol
pub struct Token {
    /// id is used for tx signature and serialization
    pub id: TokenId,
    /// Contract address of ERC20 token or Address::zero() for "ETH"
    pub address: Address,
    /// Token symbol (e.g. "ETH" or "USDC")
    pub symbol: String,
}

impl Token {
    pub fn new(id: TokenId, address: Address, symbol: &str) -> Self {
        Self {
            id,
            address,
            symbol: symbol.to_string(),
        }
    }
}

pub fn priv_key_from_fs(fs: Fs) -> PrivateKey {
    PrivateKeyImport(fs)
}

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

pub fn is_token_amount_packable(amount: &BigDecimal) -> bool {
    Some(amount.clone()) == unpack_token_amount(&pack_token_amount(amount))
}

pub fn is_fee_amount_packable(amount: &BigDecimal) -> bool {
    Some(amount.clone()) == unpack_fee_amount(&pack_fee_amount(amount))
}

pub fn unpack_token_amount(data: &[u8]) -> Option<BigDecimal> {
    unpack_float(
        data,
        params::AMOUNT_EXPONENT_BIT_WIDTH,
        params::AMOUNT_MANTISSA_BIT_WIDTH,
    )
    .map(u128_to_bigdecimal)
}

pub fn unpack_fee_amount(data: &[u8]) -> Option<BigDecimal> {
    unpack_float(
        data,
        params::FEE_EXPONENT_BIT_WIDTH,
        params::FEE_MANTISSA_BIT_WIDTH,
    )
    .map(u128_to_bigdecimal)
}

pub fn closest_packable_fee_amount(amount: &BigDecimal) -> BigDecimal {
    let fee_packed = pack_fee_amount(&amount);
    unpack_fee_amount(&fee_packed).expect("fee repacking")
}

pub fn closest_packable_token_amount(amount: &BigDecimal) -> BigDecimal {
    let fee_packed = pack_token_amount(&amount);
    unpack_token_amount(&fee_packed).expect("token amount repacking")
}

#[cfg(test)]
mod test {
    use super::*;
    use bigdecimal::BigDecimal;
    #[test]
    fn test_roundtrip() {
        let zero = BigDecimal::from(1);
        let one = BigDecimal::from(1);
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
            u128_to_bigdecimal((1u128 << params::AMOUNT_MANTISSA_BIT_WIDTH) - 1);
        let max_mantissa_fee = u128_to_bigdecimal((1u128 << params::FEE_MANTISSA_BIT_WIDTH) - 1);
        assert!(is_token_amount_packable(&max_mantissa_token));
        assert!(is_fee_amount_packable(&max_mantissa_fee));
        assert!(!is_token_amount_packable(
            &(max_mantissa_token + BigDecimal::from(1))
        ));
        assert!(!is_fee_amount_packable(
            &(max_mantissa_fee + BigDecimal::from(1))
        ));
    }

    #[test]
    fn pack_to_closest_packable() {
        let fee = BigDecimal::from(1_234_123_424);
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
            BigDecimal::from(0),
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

        let token = BigDecimal::from(123_456_789_123_456_789u64);
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
            BigDecimal::from(0),
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
