use pairing::bn256::Bn256;

/// Account tree depth
pub const ACCOUNT_TREE_DEPTH: &'static usize = &24;

/// Account subtree depth
pub const ACCOUNT_SUBTREE_DEPTH: &'static usize = &9;

/// Balance tree depth
pub const BALANCE_TREE_DEPTH: &'static usize = &8;

/// Balance tree depth
pub const SUBACCOUNT_TREE_DEPTH: &'static usize = &8;
/// Account subtree hash width
pub const SUBTREE_HASH_WIDTH: &'static usize = &256; //seems to be equal to Bn256::NUM_BITS could be replaced

/// balance bit width
pub const BALANCE_BIT_WIDTH: &'static usize = &128;

pub const NEW_PUBKEY_HASH_WIDTH: &'static usize = &160;
/// Nonce bit width
pub const NONCE_BIT_WIDTH: &'static usize = &32;

/// Block number bit width
pub const BLOCK_NUMBER_BIT_WIDTH: &'static usize = &32;

/// Amount bit widths
pub const AMOUNT_EXPONENT_BIT_WIDTH: &'static usize = &5;
pub const AMOUNT_MANTISSA_BIT_WIDTH: &'static usize = &19;

pub const COMPACT_AMOUNT_EXPONENT_BIT_WIDTH: &'static usize = &5;
pub const COMPACT_AMOUNT_MANTISSA_BIT_WIDTH: &'static usize = &11;

/// Fee bit widths
pub const FEE_EXPONENT_BIT_WIDTH: &'static usize = &5;
pub const FEE_MANTISSA_BIT_WIDTH: &'static usize = &3;

// Signature data
pub const SIGNATURE_S_BIT_WIDTH: &'static usize = &256;
pub const SIGNATURE_R_X_BIT_WIDTH: &'static usize = &256;
pub const SIGNATURE_R_Y_BIT_WIDTH: &'static usize = &256;

// Fr element encoding
pub const FR_BIT_WIDTH: &'static usize = &256;
