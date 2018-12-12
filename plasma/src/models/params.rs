pub const BALANCE_TREE_DEPTH: usize = 24;

/// Amount bit widths
pub const AMOUNT_EXPONENT_BIT_WIDTH: usize = 5;
pub const AMOUNT_MANTISSA_BIT_WIDTH: usize = 11;

/// Fee bit widths
pub const FEE_EXPONENT_BIT_WIDTH: usize = 5;
pub const FEE_MANTISSA_BIT_WIDTH: usize = 3;

pub const BALANCE_BIT_WIDTH: usize = 128;




/// Nonce bit width
pub const NONCE_BIT_WIDTH: usize = 32;

/// Block number bit width
pub const BLOCK_NUMBER_BIT_WIDTH: usize = 32;

// Signature data
pub const SIGNATURE_S_BIT_WIDTH: usize = 256;
pub const SIGNATURE_R_X_BIT_WIDTH: usize = 256;
pub const SIGNATURE_R_Y_BIT_WIDTH: usize = 256;

// Fr element encoding 
pub const FR_BIT_WIDTH: usize = 256;




// // Total public data length
// pub PUBLIC_DATA_BIT_LENGTH: usize = 2*BALANCE_TREE_DEPTH 
//                                                     + AMOUNT_EXPONENT_BIT_WIDTH 
//                                                     + AMOUNT_MANTISSA_BIT_WIDTH
//                                                     + FEE_EXPONENT_BIT_WIDTH
//                                                     + FEE_MANTISSA_BIT_WIDTH;

// // Total private data length
// pub PRIVATE_DATA_BIT_LENGTH: usize = NONCE_BIT_WIDTH 
//                                                     + SIGNATURE_S_BIT_WIDTH 
//                                                     + SIGNATURE_R_X_BIT_WIDTH
//                                                     + SIGNATURE_R_Y_BIT_WIDTH;