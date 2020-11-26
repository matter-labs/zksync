// Built-in deps
// External uses
use num::BigUint;
use serde::Deserialize;
// Workspace uses
use zksync_types::{AccountId, Address, Nonce, PubKeyHash, TokenId};
use zksync_utils::{
    BigUintSerdeAsRadix10Str, OptionBytesToHexSerde, ZeroPrefixHexSerde, ZeroxPrefix,
};
// Local uses
use super::{config_path, load_json};

#[derive(Debug, Deserialize)]
pub struct Config {
    #[serde(rename = "cryptoPrimitivesTest")]
    pub crypto_primitives: TestSet<CryptoPrimitiveInput, CryptoPrimitiveOutput>,
    #[serde(rename = "txTest")]
    pub transactions: TestSet<TxInput, TxOutput>,
    pub utils: UtilsTests,
}

impl Config {
    pub fn load() -> Self {
        let object = load_json(&config_path("sdk/test-vectors.json"));
        serde_json::from_value(object).expect("Cannot deserialize test vectors config")
    }
}

#[derive(Debug, Deserialize)]
pub struct TestSet<I, O> {
    pub description: String,
    pub items: Vec<TestEntry<I, O>>,
}

#[derive(Debug, Deserialize)]
pub struct TestEntry<I, O> {
    pub inputs: I,
    pub outputs: O,
}

#[derive(Debug, Deserialize)]
pub struct CryptoPrimitiveInput {
    #[serde(with = "ZeroPrefixHexSerde")]
    pub seed: Vec<u8>,
    #[serde(with = "ZeroPrefixHexSerde")]
    pub message: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CryptoPrimitiveOutput {
    #[serde(with = "ZeroPrefixHexSerde")]
    pub private_key: Vec<u8>,
    // FIXME: is it really a hash?
    pub pub_key_hash: String,
    pub signature: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxInput {
    #[serde(with = "ZeroPrefixHexSerde")]
    pub eth_private_key: Vec<u8>,
    #[serde(flatten)]
    pub tx: Tx,
    pub eth_sign_data: EthSignature,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum Tx {
    Transfer(Box<Transfer>),
    Withdraw(Box<Withdraw>),
    ChangePubKey(Box<ChangePubKey>),
    ForcedExit(Box<ForcedExit>),
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    pub account_id: AccountId,
    pub from: Address,
    pub to: Address,
    pub token_id: TokenId,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub amount: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub fee: BigUint,
    pub nonce: Nonce,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePubKey {
    pub account_id: AccountId,
    pub account: Address,
    pub new_pk_hash: PubKeyHash,
    pub fee_token_id: TokenId,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub fee: BigUint,
    pub nonce: Nonce,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Withdraw {
    pub account_id: AccountId,
    pub from: Address,
    pub eth_address: Address,
    pub token_id: TokenId,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub amount: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub fee: BigUint,
    pub nonce: Nonce,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForcedExit {
    pub initiator_account_id: AccountId,
    pub from: Address,
    pub target: Address,
    pub token_id: TokenId,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub fee: BigUint,
    pub nonce: Nonce,
}

#[derive(Debug, Deserialize)]
// TODO: find a way to tag the variants.
//  The field "type" cannot tag both `Tx` and `EthSignature`.
//  Thus, the variants are now sorted starting with the most constrained ones.
//  #[serde(tag = "type", content = "ethSignData")]
#[serde(untagged)]
pub enum EthSignature {
    Transfer(Box<TransferSignature>),
    Withdraw(Box<WithdrawSignature>),
    ChangePubKey(Box<ChangePubKeySignature>),
    ForcedExit,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferSignature {
    pub string_amount: String,
    pub string_token: String,
    pub string_fee: String,
    pub to: Address,
    pub account_id: AccountId,
    pub nonce: Nonce,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawSignature {
    pub string_amount: String,
    pub string_token: String,
    pub string_fee: String,
    pub eth_address: Address,
    pub account_id: AccountId,
    pub nonce: Nonce,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePubKeySignature {
    pub pub_key_hash: PubKeyHash,
    pub account_id: AccountId,
    pub nonce: Nonce,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxOutput {
    #[serde(with = "ZeroPrefixHexSerde")]
    pub sign_bytes: Vec<u8>,
    pub signature: Signature,
    pub eth_sign_message: Option<String>,
    #[serde(with = "OptionBytesToHexSerde::<ZeroxPrefix>")]
    pub eth_signature: Option<Vec<u8>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Signature {
    pub pub_key: String,
    pub signature: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UtilsTests {
    pub amount_packing: TestSet<PackingInput, PackingOutput>,
    pub fee_packing: TestSet<PackingInput, PackingOutput>,
    pub token_formatting: TestSet<TokenFormattingInput, TokenFormattingOutput>,
}

#[derive(Debug, Deserialize)]
pub struct PackingInput {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub value: BigUint,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackingOutput {
    pub packable: bool,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub closest_packable: BigUint,
    #[serde(with = "ZeroPrefixHexSerde")]
    pub packed_value: Vec<u8>,
}

#[derive(Debug, Deserialize)]
pub struct TokenFormattingInput {
    pub token: String,
    pub decimals: u8,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub amount: BigUint,
}

#[derive(Debug, Deserialize)]
pub struct TokenFormattingOutput {
    pub formatted: String,
}
