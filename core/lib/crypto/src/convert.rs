use crate::franklin_crypto::bellman::pairing::ff::{PrimeField, PrimeFieldRepr};

/// Converts the field element into a byte array.
pub fn fe_to_bytes<F: PrimeField>(value: &F) -> Vec<u8> {
    let mut buf: Vec<u8> = Vec::with_capacity(32);
    value.into_repr().write_be(&mut buf).unwrap();

    buf
}

pub fn fe_from_bytes<F: PrimeField>(value: &[u8]) -> Result<F, failure::Error> {
    let mut repr = F::Repr::default();

    // `repr.as_ref()` converts `repr` to a list of `u64`. Each element has 8 bytes,
    // so to obtain size in bytes, we multiply the array size with the size of `u64`.
    let expected_input_size = repr.as_ref().len() * 8;
    if value.len() != expected_input_size {
        failure::bail!("Incorrect input size")
    }
    repr.read_be(value)
        .map_err(|e| failure::format_err!("Cannot parse value {:?}: {}", value, e))?;
    F::from_repr(repr).map_err(|e| {
        failure::format_err!("Cannot convert into prime field value {:?}: {}", value, e)
    })
}

/// Returns hex representation of the field element without `0x` prefix.
pub fn fe_to_hex<F: PrimeField>(value: &F) -> String {
    let mut buf: Vec<u8> = Vec::with_capacity(32);
    value.into_repr().write_be(&mut buf).unwrap();
    hex::encode(&buf)
}

pub fn fe_from_hex<F: PrimeField>(value: &str) -> Result<F, failure::Error> {
    let value = if value.starts_with("0x") {
        &value[2..]
    } else {
        value
    };

    // Buffer is reversed and read as little endian, since we pad it with zeros to
    // match the expected length.
    let mut buf = hex::decode(&value)
        .map_err(|e| failure::format_err!("could not decode hex: {}, reason: {}", value, e))?;
    buf.reverse();
    let mut repr = F::Repr::default();

    // `repr.as_ref()` converts `repr` to a list of `u64`. Each element has 8 bytes,
    // so to obtain size in bytes, we multiply the array size with the size of `u64`.
    buf.resize(repr.as_ref().len() * 8, 0);
    repr.read_le(&buf[..])
        .map_err(|e| failure::format_err!("could not read {}: {}", value, e))?;
    F::from_repr(repr)
        .map_err(|e| failure::format_err!("could not convert into prime field: {}: {}", value, e))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::Fr;
    use crypto_exports::rand::{Rand, SeedableRng, XorShiftRng};

    /// Checks that converting FE to the hex form and back results
    /// in the same FE.
    #[test]
    fn fe_hex_roundtrip() {
        let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);

        let fr = Fr::rand(&mut rng);

        let encoded_fr = fe_to_hex(&fr);
        let decoded_fr = fe_from_hex(&encoded_fr).expect("Can't decode encoded fr");

        assert_eq!(fr, decoded_fr);
    }

    /// Checks that converting FE to the bytes form and back results
    /// in the same FE.
    #[test]
    fn fe_bytes_roundtrip() {
        let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);

        let fr = Fr::rand(&mut rng);

        let encoded_fr = fe_to_bytes(&fr);
        let decoded_fr = fe_from_bytes(&encoded_fr).expect("Can't decode encoded fr");

        assert_eq!(fr, decoded_fr);
    }
}
