// Built-in deps
// External deps
use crate::franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use lazy_static::lazy_static;
// Workspace deps
use crate::franklin_crypto::rescue::bn256::Bn256RescueParams;
use crate::merkle_tree::rescue_hasher::BabyRescueHasher;
use zksync_basic_types::{AccountId, TokenId};

/// Depth of the account tree.
pub const ACCOUNT_TREE_DEPTH: usize = 32;
/// Depth of the balance tree for each account.
pub const BALANCE_TREE_DEPTH: usize = 11;

/// account_tree_depth.
pub fn account_tree_depth() -> usize {
    ACCOUNT_TREE_DEPTH
}

/// balance tree_depth.
pub fn balance_tree_depth() -> usize {
    BALANCE_TREE_DEPTH
}

/// Number of supported tokens.
pub fn total_tokens() -> usize {
    2usize.pow(balance_tree_depth() as u32)
}

/// Number of tokens that are processed by this release
pub fn number_of_processable_tokens() -> usize {
    let num = 128;

    assert!(num <= total_tokens());
    assert!(num.is_power_of_two());

    num
}

/// Depth of the left subtree of the account tree that can be used in the current version of the circuit.
pub fn used_account_subtree_depth() -> usize {
    let num = 24; // total accounts = 2.pow(num) ~ 16mil

    assert!(num <= account_tree_depth());

    num
}

/// Max token id, based on the depth of the used left subtree
pub fn max_account_id() -> AccountId {
    let list_count = 2u32.saturating_pow(used_account_subtree_depth() as u32);
    if list_count == u32::max_value() {
        list_count
    } else {
        list_count - 1
    }
}

/// Max token id, based on the number of processable tokens
pub fn max_token_id() -> TokenId {
    number_of_processable_tokens() as u16 - 1
}

pub const ETH_TOKEN_ID: TokenId = 0;

pub const ACCOUNT_ID_BIT_WIDTH: usize = 32;

pub const INPUT_DATA_ADDRESS_BYTES_WIDTH: usize = 32;
pub const INPUT_DATA_BLOCK_NUMBER_BYTES_WIDTH: usize = 32;
pub const INPUT_DATA_FEE_ACC_BYTES_WIDTH_WITH_EMPTY_OFFSET: usize = 32;
pub const INPUT_DATA_FEE_ACC_BYTES_WIDTH: usize = 3;
pub const INPUT_DATA_ROOT_BYTES_WIDTH: usize = 32;
pub const INPUT_DATA_EMPTY_BYTES_WIDTH: usize = 64;
pub const INPUT_DATA_ROOT_HASH_BYTES_WIDTH: usize = 32;

pub const TOKEN_BIT_WIDTH: usize = 16;
pub const TX_TYPE_BIT_WIDTH: usize = 8;

/// Account subtree hash width
pub const SUBTREE_HASH_WIDTH: usize = 254; //seems to be equal to Bn256::NUM_BITS could be replaced
pub const SUBTREE_HASH_WIDTH_PADDED: usize = 256;

/// balance bit width
pub const BALANCE_BIT_WIDTH: usize = 128;

pub const NEW_PUBKEY_HASH_WIDTH: usize = FR_ADDRESS_LEN * 8;
pub const ADDRESS_WIDTH: usize = FR_ADDRESS_LEN * 8;
/// Nonce bit width
pub const NONCE_BIT_WIDTH: usize = 32;
//
pub const CHUNK_BIT_WIDTH: usize = 72;
pub const CHUNK_BYTES: usize = CHUNK_BIT_WIDTH / 8;

pub const MAX_CIRCUIT_MSG_HASH_BITS: usize = 736;

pub const ETH_ADDRESS_BIT_WIDTH: usize = 160;
/// Block number bit width
pub const BLOCK_NUMBER_BIT_WIDTH: usize = 32;

/// Amount bit widths
pub const AMOUNT_EXPONENT_BIT_WIDTH: usize = 5;
pub const AMOUNT_MANTISSA_BIT_WIDTH: usize = 35;

/// Fee bit widths
pub const FEE_EXPONENT_BIT_WIDTH: usize = 5;
pub const FEE_MANTISSA_BIT_WIDTH: usize = 11;

// Signature data
pub const SIGNATURE_S_BIT_WIDTH: usize = 254;
pub const SIGNATURE_S_BIT_WIDTH_PADDED: usize = 256;
pub const SIGNATURE_R_X_BIT_WIDTH: usize = 254;
pub const SIGNATURE_R_Y_BIT_WIDTH: usize = 254;
pub const SIGNATURE_R_BIT_WIDTH_PADDED: usize = 256;

// Fr element encoding
pub const FR_BIT_WIDTH: usize = 254;
pub const FR_BIT_WIDTH_PADDED: usize = 256;

pub const LEAF_DATA_BIT_WIDTH: usize =
    NONCE_BIT_WIDTH + NEW_PUBKEY_HASH_WIDTH + FR_BIT_WIDTH_PADDED + ETH_ADDRESS_BIT_WIDTH;

/// Priority op should be executed for this number of eth blocks.
pub const PRIORITY_EXPIRATION: u64 = 35000;
pub const FR_ADDRESS_LEN: usize = 20;

pub const PAD_MSG_BEFORE_HASH_BITS_LEN: usize = 736;

/// Size of the data that is signed for withdraw tx
pub const SIGNED_WITHDRAW_BIT_WIDTH: usize = TX_TYPE_BIT_WIDTH
    + ACCOUNT_ID_BIT_WIDTH
    + 2 * ADDRESS_WIDTH
    + TOKEN_BIT_WIDTH
    + BALANCE_BIT_WIDTH
    + FEE_EXPONENT_BIT_WIDTH
    + FEE_MANTISSA_BIT_WIDTH
    + NONCE_BIT_WIDTH;

/// Size of the data that is signed for transfer tx
pub const SIGNED_TRANSFER_BIT_WIDTH: usize = TX_TYPE_BIT_WIDTH
    + ACCOUNT_ID_BIT_WIDTH
    + 2 * ADDRESS_WIDTH
    + TOKEN_BIT_WIDTH
    + AMOUNT_EXPONENT_BIT_WIDTH
    + AMOUNT_MANTISSA_BIT_WIDTH
    + FEE_EXPONENT_BIT_WIDTH
    + FEE_MANTISSA_BIT_WIDTH
    + NONCE_BIT_WIDTH;

/// Size of the data that is signed for forced exit tx
pub const SIGNED_FORCED_EXIT_BIT_WIDTH: usize = TX_TYPE_BIT_WIDTH
    + ACCOUNT_ID_BIT_WIDTH
    + ADDRESS_WIDTH
    + TOKEN_BIT_WIDTH
    + FEE_EXPONENT_BIT_WIDTH
    + FEE_MANTISSA_BIT_WIDTH
    + NONCE_BIT_WIDTH;

/// Size of the data that is signed for change pubkey tx
pub const SIGNED_CHANGE_PUBKEY_BIT_WIDTH: usize = TX_TYPE_BIT_WIDTH
    + ACCOUNT_ID_BIT_WIDTH
    + ADDRESS_WIDTH
    + NEW_PUBKEY_HASH_WIDTH
    + TOKEN_BIT_WIDTH
    + FEE_EXPONENT_BIT_WIDTH
    + FEE_MANTISSA_BIT_WIDTH
    + NONCE_BIT_WIDTH;

lazy_static! {
    pub static ref JUBJUB_PARAMS: AltJubjubBn256 = AltJubjubBn256::new();
    pub static ref RESCUE_PARAMS: Bn256RescueParams = Bn256RescueParams::new_checked_2_into_1();
    pub static ref RESCUE_HASHER: BabyRescueHasher = BabyRescueHasher::default();
}
