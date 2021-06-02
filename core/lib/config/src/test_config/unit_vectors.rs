// Built-in deps
// External uses
use num::BigUint;
use serde::Deserialize;
// Workspace uses
use zksync_types::{AccountId, Address, Nonce, PubKeyHash, TokenId, H256};
use zksync_utils::{
    BigUintSerdeAsRadix10Str, OptionBytesToHexSerde, ZeroPrefixHexSerde, ZeroxPrefix,
};
// Local uses
use super::{config_path, load_json};
use zksync_types::tx::TimeRange;

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
    pub pub_key: String,
    pub signature: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxInput {
    #[serde(with = "ZeroPrefixHexSerde")]
    pub eth_private_key: Vec<u8>,
    #[serde(flatten)]
    pub data: TxData,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum TxData {
    #[serde(rename_all = "camelCase")]
    Transfer {
        data: Box<Transfer>,
        eth_sign_data: TransferSignatureInputs,
    },
    #[serde(rename_all = "camelCase")]
    Withdraw {
        data: Box<Withdraw>,
        eth_sign_data: WithdrawSignatureInputs,
    },
    #[serde(rename_all = "camelCase")]
    ChangePubKey {
        data: Box<ChangePubKey>,
        eth_sign_data: ChangePubKeySignatureInputs,
    },
    #[serde(rename_all = "camelCase")]
    ForcedExit { data: Box<ForcedExit> },
    #[serde(rename_all = "camelCase")]
    WithdrawNFT {
        data: Box<WithdrawNFT>,
        eth_sign_data: WithdrawNFTSignatureInputs,
    },
    #[serde(rename_all = "camelCase")]
    MintNFT {
        data: Box<MintNFT>,
        eth_sign_data: MintNFTSignatureInputs,
    },
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawNFT {
    pub account_id: AccountId,
    pub from: Address,
    pub to: Address,
    pub token_id: TokenId,
    pub fee_token_id: TokenId,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub fee: BigUint,
    pub nonce: Nonce,
    #[serde(flatten)]
    pub time_range: TimeRange,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MintNFT {
    pub creator_id: AccountId,
    pub creator_address: Address,
    pub recipient: Address,
    pub content_hash: H256,
    pub fee_token_id: TokenId,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub fee: BigUint,
    pub nonce: Nonce,
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
    #[serde(flatten)]
    pub time_range: TimeRange,
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
    #[serde(flatten)]
    pub time_range: TimeRange,
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
    #[serde(flatten)]
    pub time_range: TimeRange,
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
    #[serde(flatten)]
    pub time_range: TimeRange,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawNFTSignatureInputs {
    pub token: TokenId,
    pub to: Address,
    pub string_fee: String,
    pub string_fee_token: String,
    pub nonce: Nonce,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MintNFTSignatureInputs {
    pub string_fee_token: String,
    pub string_fee: String,
    pub recipient: Address,
    pub content_hash: H256,
    pub nonce: Nonce,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferSignatureInputs {
    pub string_amount: String,
    pub string_token: String,
    pub string_fee: String,
    pub to: Address,
    pub account_id: AccountId,
    pub nonce: Nonce,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawSignatureInputs {
    pub string_amount: String,
    pub string_token: String,
    pub string_fee: String,
    pub eth_address: Address,
    pub account_id: AccountId,
    pub nonce: Nonce,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePubKeySignatureInputs {
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
    #[serde(with = "OptionBytesToHexSerde::<ZeroxPrefix>")]
    pub eth_sign_message: Option<Vec<u8>>,
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
