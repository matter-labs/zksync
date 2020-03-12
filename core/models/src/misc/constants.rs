/// Ethereum ECDSA signature size.
pub const ETH_SIGNATURE_LENGTH: usize = 65;
/// Size of hex representation in form of "0x{...}".
/// Two bytes for "0x", and two for each byte of the signature.
pub const ETH_SIGNATURE_HEX_LENGTH: usize = (ETH_SIGNATURE_LENGTH * 2) + 2;
