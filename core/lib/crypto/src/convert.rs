use crate::{
    error::ConversionError,
    franklin_crypto::bellman::pairing::ff::{PrimeField, PrimeFieldRepr},
};

/// Extension trait denoting common conversion method for field elements.
pub trait FeConvert: PrimeField {
    /// Converts the field element into a byte array.
    fn to_bytes(&self) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::with_capacity(32);
        self.into_repr().write_be(&mut buf).unwrap();

        buf
    }

    /// Reads a field element from its byte sequence representation.
    fn from_bytes(value: &[u8]) -> Result<Self, ConversionError> {
        let mut repr = Self::Repr::default();

        // `repr.as_ref()` converts `repr` to a list of `u64`. Each element has 8 bytes,
        // so to obtain size in bytes, we multiply the array size with the size of `u64`.
        let expected_input_size = repr.as_ref().len() * 8;
        if value.len() != expected_input_size {
            return Err(ConversionError::IncorrectInputSize {
                size: value.len(),
                expected_size: expected_input_size,
            });
        }
        repr.read_be(value).map_err(ConversionError::ParsingError)?;
        Self::from_repr(repr).map_err(From::from)
    }

    /// Returns hex representation of the field element without `0x` prefix.
    fn to_hex(&self) -> String {
        let mut buf: Vec<u8> = Vec::with_capacity(32);
        self.into_repr().write_be(&mut buf).unwrap();
        hex::encode(&buf)
    }

    /// Reads a field element from its hexadecimal representation.
    fn from_hex(value: &str) -> Result<Self, ConversionError> {
        let value = if let Some(value) = value.strip_prefix("0x") {
            value
        } else {
            value
        };

        // Buffer is reversed and read as little endian, since we pad it with zeros to
        // match the expected length.
        let mut buf = hex::decode(&value)?;
        buf.reverse();
        let mut repr = Self::Repr::default();

        // `repr.as_ref()` converts `repr` to a list of `u64`. Each element has 8 bytes,
        // so to obtain size in bytes, we multiply the array size with the size of `u64`.
        buf.resize(repr.as_ref().len() * 8, 0);
        repr.read_le(&buf[..])
            .map_err(ConversionError::ParsingError)?;
        Self::from_repr(repr).map_err(From::from)
    }
}

impl<T> FeConvert for T where T: PrimeField {}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::{
        rand::{Rand, SeedableRng, XorShiftRng},
        Fr,
    };

    /// Checks that converting FE to the hex form and back results
    /// in the same FE.
    #[test]
    fn fe_hex_roundtrip() {
        let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);

        let fr = Fr::rand(&mut rng);

        let encoded_fr = fr.to_hex();
        let decoded_fr = Fr::from_hex(&encoded_fr).expect("Can't decode encoded fr");

        assert_eq!(fr, decoded_fr);
    }

    /// Checks that converting FE to the bytes form and back results
    /// in the same FE.
    #[test]
    fn fe_bytes_roundtrip() {
        let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);

        let fr = Fr::rand(&mut rng);

        let encoded_fr = fr.to_bytes();
        let decoded_fr = Fr::from_bytes(&encoded_fr).expect("Can't decode encoded fr");

        assert_eq!(fr, decoded_fr);
    }
}
