/// Ethereum ECDSA signature size.
pub const ETH_SIGNATURE_LENGTH: usize = 65;
/// Size of hex representation in form of "0x{...}".
/// Two bytes for "0x", and two for each byte of the signature.
pub const ETH_SIGNATURE_HEX_LENGTH: usize = (ETH_SIGNATURE_LENGTH * 2) + 2;

/// isValidSignature return value according to EIP1271 standard
/// bytes4(keccak256("isValidSignature(bytes,bytes)")
pub const EIP1271_SUCCESS_RETURN_VALUE: [u8; 4] = [0x20, 0xc1, 0x3b, 0x0b];
