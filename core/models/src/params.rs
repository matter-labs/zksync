// Built-in uses
use std::env;
use std::str::FromStr;
// External uses
use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use lazy_static::lazy_static;
// Workspace uses
use crate::merkle_tree::pedersen_hasher::BabyPedersenHasher;
use crate::node::TokenId;

static mut ACCOUNT_TREE_DEPTH_VALUE: usize = 0;
/// account_tree_depth.
/// Value must be specified as environment variable at compile time under `ACCOUNT_TREE_DEPTH_VALUE` key.
pub fn account_tree_depth() -> usize {
    // use of mutable static is unsafe as it can be mutated by multiple threads.
    // There's no risk of data race, the worst that can happen is that we parse
    // and set environment value multuple times, which is ok.
    unsafe {
        if ACCOUNT_TREE_DEPTH_VALUE == 0 {
            let value: &'static str = env!("ACCOUNT_TREE_DEPTH");
            ACCOUNT_TREE_DEPTH_VALUE =
                usize::from_str_radix(value, 10).expect("account tree depth value is invalid");
            let runtime_value = env::var("ACCOUNT_TREE_DEPTH").expect("ACCOUNT_TREE_DEPTH missing");
            let runtime_value =
                usize::from_str(&runtime_value).expect("ACCOUNT_TREE_DEPTH invalid");
            if runtime_value != ACCOUNT_TREE_DEPTH_VALUE {
                panic!(
                    "ACCOUNT_TREE_DEPTH want runtime value: {}, got: {}",
                    ACCOUNT_TREE_DEPTH_VALUE, runtime_value
                );
            }
        }
        ACCOUNT_TREE_DEPTH_VALUE
    }
}
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

pub const MAX_CIRCUIT_PEDERSEN_HASH_BITS: usize = 736;

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

static mut BLOCK_SIZE_CHUNKS_VALUE: usize = 0;
/// block_size_chunks.
/// Value must be specified as environment variable at compile time under `BLOCK_SIZE_CHUNKS` key.
pub fn block_size_chunks() -> usize {
    // use of mutable static is unsafe as it can be mutated by multiple threads.
    // using `unsafe` block as there's no risk of data race, the worst that can
    // happen is we read and set environment value multuple times, which is ok.
    unsafe {
        if BLOCK_SIZE_CHUNKS_VALUE == 0 {
            let value: &'static str = env!("BLOCK_SIZE_CHUNKS");
            BLOCK_SIZE_CHUNKS_VALUE =
                usize::from_str_radix(value, 10).expect("block size chunks value is invalid");
            let runtime_value = env::var("BLOCK_SIZE_CHUNKS").expect("BLOCK_SIZE_CHUNKS missing");
            let runtime_value = usize::from_str(&runtime_value).expect("BLOCK_SIZE_CHUNKS invalid");
            if runtime_value != BLOCK_SIZE_CHUNKS_VALUE {
                panic!(
                    "BLOCK_SIZE_CHUNKS want runtime value: {}, got: {}",
                    BLOCK_SIZE_CHUNKS_VALUE, runtime_value
                );
            }
        }
        BLOCK_SIZE_CHUNKS_VALUE
    }
}

/// Priority op should be executed for this number of eth blocks.
pub const PRIORITY_EXPIRATION: u64 = 250;
pub const FR_ADDRESS_LEN: usize = 20;

pub const KEY_FILENAME: &str = "franklin_pk.key";

pub const PAD_MSG_BEFORE_HASH_BITS_LEN: usize = 736;

lazy_static! {
    pub static ref JUBJUB_PARAMS: AltJubjubBn256 = AltJubjubBn256::new();
    pub static ref PEDERSEN_HASHER: BabyPedersenHasher = BabyPedersenHasher::default();
}
