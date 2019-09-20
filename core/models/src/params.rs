use crate::merkle_tree::pedersen_hasher::BabyPedersenHasher;
use crate::node::TokenId;
use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use lazy_static::lazy_static;

/// Account tree depth
pub const ACCOUNT_TREE_DEPTH: usize = 5;
pub const ACCOUNT_ID_BIT_WIDTH: usize = 24;

/// Balance tree depth
pub const BALANCE_TREE_DEPTH: usize = 5;
pub const TOKEN_BIT_WIDTH: usize = 16;

/// Account tree depth
pub const TX_TYPE_BIT_WIDTH: usize = 8;

/// Account subtree hash width
pub const SUBTREE_HASH_WIDTH: usize = 254; //seems to be equal to Bn256::NUM_BITS could be replaced

/// balance bit width
pub const BALANCE_BIT_WIDTH: usize = 128;

pub const NEW_PUBKEY_HASH_WIDTH: usize = FR_ADDRESS_LEN * 8;
/// Nonce bit width
pub const NONCE_BIT_WIDTH: usize = 32;
//
//
pub const CHUNK_BIT_WIDTH: usize = 64;

pub const MAX_CIRCUIT_PEDERSEN_HASH_BITS: usize = 738;

pub const ETHEREUM_KEY_BIT_WIDTH: usize = 160;
/// Block number bit width
pub const BLOCK_NUMBER_BIT_WIDTH: usize = 32;

/// Amount bit widths
pub const AMOUNT_EXPONENT_BIT_WIDTH: usize = 5;
pub const AMOUNT_MANTISSA_BIT_WIDTH: usize = 19;

/// Fee bit widths
pub const FEE_EXPONENT_BIT_WIDTH: usize = 6;
pub const FEE_MANTISSA_BIT_WIDTH: usize = 10;

// Signature data
pub const SIGNATURE_S_BIT_WIDTH: usize = 254;
pub const SIGNATURE_R_X_BIT_WIDTH: usize = 254;
pub const SIGNATURE_R_Y_BIT_WIDTH: usize = 254;

// Fr element encoding
pub const FR_BIT_WIDTH: usize = 254;
pub const FR_BIT_WIDTH_PADDED: usize = 256;

/// Number of supported tokens.
pub const TOTAL_TOKENS: usize = 1 << BALANCE_TREE_DEPTH;
pub const ETH_TOKEN_ID: TokenId = 0;

pub const BLOCK_SIZE_CHUNKS: usize = 18;

/// Priority op should be executed for this number of eth blocks.
pub const PRIORITY_EXPIRATION: u64 = 250;
pub const FR_ADDRESS_LEN: usize = 20;

pub const KEY_FILENAME: &str = "franklin_pk.key";

lazy_static! {
    pub static ref JUBJUB_PARAMS: AltJubjubBn256 = AltJubjubBn256::new();
    pub static ref PEDERSEN_HASHER: BabyPedersenHasher = BabyPedersenHasher::default();
}
